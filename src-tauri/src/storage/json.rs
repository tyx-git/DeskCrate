use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use super::{mutate_snapshot, DataRepository, SettingsRepository};
use crate::{
    error::{AppResult, JSON_PARSE_ERROR_CODE},
    models::{AppData, AppError, Settings},
};
use chrono::Utc;
use fs2::FileExt;
use uuid::Uuid;

// 锁等待上限。锁的拥有者崩溃时操作系统会释放文件锁；上限用于避免
// 文件损坏、权限变化或外部程序长期占锁时让前端请求永久阻塞。
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone)]
pub struct JsonStore {
    path: PathBuf,
    config_path: PathBuf,
    data_path: PathBuf,
    icons_path: PathBuf,
    config_lock_path: PathBuf,
    data_lock_path: PathBuf,
}

impl JsonStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        // 逐段 join 而不是一次写 `"config/config.json"`：后者会把正斜杠原样留在
        // 路径里，Windows 上虽然能用，但这个路径会出现在用户可见的报错里
        // （"…\debug\config/data.json 不是合法的 JSON"），看着像是拼接错了。
        let config_dir = path.join("config");
        let config_path = config_dir.join("config.json");
        let data_path = config_dir.join("data.json");
        let icons_path = path.join("icons");
        // 锁文件独立于 JSON 文件。原子替换会更换 JSON 文件本身，
        // 因此不能把锁绑定在会被替换的目标文件句柄上。
        let config_lock_path = config_dir.join("config.json.lock");
        let data_lock_path = config_dir.join("data.json.lock");
        Self {
            path,
            config_path,
            data_path,
            icons_path,
            config_lock_path,
            data_lock_path,
        }
    }
    // 创建程序运行所需的目录和两个 JSON 文件。
    //
    // 窗口首次居中、配置状态迁移等业务由 `services::window` 负责；
    // 这里仅保证存储布局存在，避免存储层和窗口层互相耦合。
    pub fn ensure_layout(&self) -> AppResult<()> {
        let config_dir = self.path.join("config");
        let icons_dir = self.path.join("icons");
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&icons_dir)?;
        // 存在性检查必须在同一把锁内重新执行，否则两个进程可能同时
        // 判断文件不存在，并用默认值覆盖彼此的初始化结果。
        self.ensure_file(&self.data_path, &self.data_lock_path, &AppData::default())?;
        self.ensure_file(
            &self.config_path,
            &self.config_lock_path,
            &Settings::default(),
        )?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn data_path(&self) -> &Path {
        &self.data_path
    }

    pub fn icons_path(&self) -> &Path {
        &self.icons_path
    }

    // 加锁lock并执行一个操作operation()
    fn with_file_lock<T, F>(&self, lock_path: &Path, operation: F) -> AppResult<T>
    where
        F: FnOnce() -> AppResult<T>,
    {
        let _lock = FileLock::acquire(lock_path)?;
        operation()
    }

    fn ensure_file<T>(&self, file_path: &Path, lock_path: &Path, default_value: &T) -> AppResult<()>
    where
        T: serde::Serialize,
    {
        self.with_file_lock(lock_path, || {
            if !file_path.exists() {
                Self::write_json(file_path, default_value)?;
            }
            Ok(())
        })
    }

    // 读取配置；内容损坏时备份原文件并按默认配置重建。
    //
    // 只有配置文件享受这个待遇，因为它的内容全部可以重新推导：窗口几何由当前
    // 工作区算得出来，主题和视图模式都有默认值。用户手工改坏 config.json 之后，
    // 应用会回到「首次启动」状态，而不是卡在一个既收不起来、尺寸也不对的半坏
    // 状态 —— 文档里「手工改配置把窗口找回来」那条自救路径因此才真正成立。
    //
    // 必须同时覆盖「读」和「读改写」两条路径：收起/展开走的是
    // `mutate_settings`，只修 `load_settings` 的话窗口照样收不起来。
    fn read_settings_or_rebuild(&self) -> AppResult<Settings> {
        match Self::read_json(&self.config_path) {
            Ok(settings) => Ok(settings),
            Err(error) if is_corrupt_json(&error) => {
                let backup = quarantine(&self.config_path)?;
                log::warn!(
                    "配置文件内容已损坏，已备份为 {} 并按默认配置重建：{error}",
                    backup.display()
                );
                let settings = Settings::default();
                Self::write_json(&self.config_path, &settings)?;
                Ok(settings)
            }
            Err(error) => Err(error),
        }
    }

    // 读取数据文件；内容损坏时**不重建**，只留一份副本并把错误说清楚。
    //
    // 与配置相反：data.json 是用户自己攒的分类和应用，静默换一份空的会看起来像
    // 「数据全没了」，比报错严重得多。副本用**复制**而不是改名 —— 原文件必须留在
    // 原位，是删掉还是修好由用户决定。
    // 读取目录数据，并在必要时把旧版结构折成当前形状。
    //
    // 根节点曾经是「顶层应用一个数组 + 分类一个数组、应用固定在前」，现在是
    // 一个可交错的有序列表。`AppData::normalize()` 负责这个转换且是幂等的，
    // 所以放在这里最合适 —— 无论从哪条路径读（`load` 还是 `mutate`），业务层
    // 拿到的都只会是同一种形状。
    fn read_data_or_report(&self) -> AppResult<AppData> {
        match Self::read_json::<AppData>(&self.data_path) {
            Ok(mut data) => {
                data.normalize();
                Ok(data)
            }
            Err(error) if is_corrupt_json(&error) => {
                let backup = copy_aside(&self.data_path)?;
                log::warn!(
                    "数据文件内容已损坏，已复制副本到 {}：{error}",
                    backup.display()
                );
                let message = format!(
                    "{error}。已复制一份到 {}；请备份或修好原文件后重试",
                    backup.display()
                );
                Err(AppError::new(error.code, message))
            }
            Err(error) => Err(error),
        }
    }

    fn write_locked<T>(&self, file_path: &Path, lock_path: &Path, value: &T) -> AppResult<()>
    where
        T: serde::Serialize,
    {
        self.with_file_lock(lock_path, || Self::write_json(file_path, value))
    }

    fn read_json<T>(path: &Path) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        // 路径必须进错误信息：这些错误会一路传到前端 toast，而 serde 的原始报错
        // （"expected value at line 1 column 3"）不含文件名 —— 用户看到之后完全
        // 不知道该去修哪个文件。
        let content = fs::read_to_string(path).map_err(|error| with_path(error.into(), path))?;
        serde_json::from_str(&content).map_err(|error| with_path(error.into(), path))
    }
    fn write_json<T>(path: &Path, value: &T) -> AppResult<()>
    where
        T: serde::Serialize,
    {
        let content = serde_json::to_string_pretty(value)?;
        atomic_write(path, content.as_bytes())
    }
}

// 把文件路径补进错误信息，保留原有错误码。
fn with_path(error: AppError, path: &Path) -> AppError {
    AppError::new(error.code, format!("{}：{}", path.display(), error.message))
}

// 判断一个错误是不是「文件内容不是合法 JSON」。
//
// 只有这一类失败可以安全地动用户的文件：内容已经坏了，备份或复制都不会损失什么。
// IO 错误（被占用、权限不足）不走这条分支。
fn is_corrupt_json(error: &AppError) -> bool {
    error.code == JSON_PARSE_ERROR_CODE
}

// 把损坏的文件改名到一边，返回备份路径。
//
// 用改名而不是删除：用户的内容可能还有参考价值，而且原路径必须空出来给重建的
// 默认值。时间戳用 `%Y%m%d-%H%M%S` 而不是 RFC 3339 —— 后者带冒号和加号，在
// Windows 上不是合法文件名字符。重建之后文件就是合法的了，所以这个名字不会堆积。
fn quarantine(path: &Path) -> AppResult<PathBuf> {
    let backup = corrupt_backup_path(
        path,
        &format!(".corrupt-{}", Utc::now().format("%Y%m%d-%H%M%S")),
    )?;
    fs::rename(path, &backup)?;
    Ok(backup)
}

// 把损坏的文件复制一份（不改名），返回副本路径。
//
// 副本名固定而不带时间戳：原文件还在，每次启动都会再走一遍这条路径，带时间戳会让
// 目录里堆满内容相同的副本。
fn copy_aside(path: &Path) -> AppResult<PathBuf> {
    let backup = corrupt_backup_path(path, ".corrupt")?;
    fs::copy(path, &backup)?;
    Ok(backup)
}

/// 损坏文件的副本名：原名字后面接一个后缀，落在同一个目录里。
fn corrupt_backup_path(path: &Path, suffix: &str) -> AppResult<PathBuf> {
    Ok(path.with_file_name(format!("{}{suffix}", file_name(path)?)))
}

fn file_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| AppError::new("json_name_invalid", "JSON 文件名无效"))
}

// 跨进程文件锁的 RAII 封装。
//
// 锁文件本身可以一直留在磁盘上；真正的锁状态属于这个 File 句柄，
// 句柄关闭或进程退出后由操作系统自动释放，不需要删除锁文件。
struct FileLock {
    file: File,
    path: PathBuf,
}

impl FileLock {
    fn acquire(path: &Path) -> AppResult<Self> {
        let parent = path.parent().ok_or_else(|| {
            crate::models::AppError::new("storage_lock_path_invalid", "锁文件没有有效的父目录")
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            crate::models::AppError::new(
                "storage_lock_open_failed",
                format!("无法创建锁文件目录 {}: {error}", parent.display()),
            )
        })?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| {
                crate::models::AppError::new(
                    "storage_lock_open_failed",
                    format!("无法打开锁文件 {}: {error}", path.display()),
                )
            })?;

        let started = Instant::now();
        loop {
            match FileExt::try_lock_exclusive(&file) {
                Ok(()) => {
                    return Ok(Self {
                        file,
                        path: path.to_path_buf(),
                    });
                }
                Err(error) if is_lock_contention(&error) => {
                    if started.elapsed() >= LOCK_TIMEOUT {
                        return Err(crate::models::AppError::new(
                            "storage_lock_timeout",
                            format!("等待锁文件超时: {}", path.display()),
                        ));
                    }
                    thread::sleep(LOCK_RETRY_INTERVAL);
                }
                Err(error) => {
                    return Err(crate::models::AppError::new(
                        "storage_lock_failed",
                        format!("无法锁定文件 {}: {error}", path.display()),
                    ));
                }
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // Drop 无法返回错误；记录失败并继续释放 File，让操作系统完成
        // 句柄级别的清理。正常路径下 unlock 不会失败。
        if let Err(error) = FileExt::unlock(&self.file) {
            log::warn!("释放 JSON 锁文件 {} 失败: {error}", self.path.display());
        }
    }
}

fn is_lock_contention(error: &std::io::Error) -> bool {
    let expected = fs2::lock_contended_error();

    match (error.raw_os_error(), expected.raw_os_error()) {
        (Some(actual), Some(expected)) => actual == expected,
        _ => error.kind() == expected.kind(),
    }
}

// 将完整 JSON 内容写入同目录临时文件，再替换目标文件。
//
// 临时文件和目标文件位于同一目录，保证替换操作发生在同一文件系统
// 卷上。Windows 下使用 `MoveFileExW` 的 replace 标志完成原子替换；
// 若目标文件尚不存在，则直接重命名临时文件。
fn atomic_write(path: &Path, content: &[u8]) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        crate::models::AppError::new("json_parent_missing", "JSON 文件没有有效的父目录")
    })?;

    fs::create_dir_all(parent)?;

    let temp_path = temporary_path(path)?;
    let mut temp_created = false;
    let write_result = (|| -> AppResult<()> {
        // create_new 防止临时文件名冲突导致覆盖其他写入者的临时内容。
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        temp_created = true;
        file.write_all(content)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);

        replace_file(&temp_path, path)
    })();

    if temp_created && write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn temporary_path(path: &Path) -> AppResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| crate::models::AppError::new("json_name_invalid", "JSON 文件名无效"))?;
    Ok(path.with_file_name(format!(".{file_name}.{}.tmp", Uuid::new_v4())))
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> AppResult<()> {
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };

    let source = wide_path(source);
    let destination = wide_path(destination);
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| {
            crate::models::AppError::new(
                "json_replace_failed",
                format!("替换 JSON 文件失败: {error:?}"),
            )
        })?;
    }

    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> AppResult<()> {
    fs::rename(source, destination).map_err(Into::into)
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

impl DataRepository for JsonStore {
    fn load_data(&self) -> AppResult<AppData> {
        self.with_file_lock(&self.data_lock_path, || self.read_data_or_report())
    }
    fn save_data(&self, value: &AppData) -> AppResult<()> {
        self.write_locked(&self.data_path, &self.data_lock_path, value)
    }

    fn mutate_data<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut AppData) -> AppResult<R>,
    {
        // 读取、业务修改和原子替换必须共享同一把跨进程锁；否则两个
        // 进程都基于旧快照修改时，后写入者会丢失前一个进程的更新。
        //
        // 读取走 `read_data_or_report` 而不是裸 `read_json`，这样数据文件损坏时
        // 同样会先留下副本，并给出「哪个文件坏了、副本在哪」的可操作错误。
        self.with_file_lock(&self.data_lock_path, || {
            mutate_snapshot(
                || self.read_data_or_report(),
                |data| Self::write_json(&self.data_path, data),
                operation,
            )
        })
    }
}

impl SettingsRepository for JsonStore {
    fn load_settings(&self) -> AppResult<Settings> {
        self.with_file_lock(&self.config_lock_path, || self.read_settings_or_rebuild())
    }
    fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.write_locked(&self.config_path, &self.config_lock_path, settings)
    }

    fn mutate_settings<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut Settings) -> AppResult<R>,
    {
        // 与 `mutate_data` 同理，读取必须走带恢复的版本：收起/展开、保存几何、
        // 补写用户尺寸都经由这条路径，只修 `load_settings` 的话配置一旦损坏，
        // 窗口就再也收不起来了。
        self.with_file_lock(&self.config_lock_path, || {
            mutate_snapshot(
                || self.read_settings_or_rebuild(),
                |settings| Self::write_json(&self.config_path, settings),
                operation,
            )
        })
    }
}
