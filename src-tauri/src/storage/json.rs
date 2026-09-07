use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use super::{mutate_snapshot, DataRepository, SettingsRepository};
use crate::{
    error::AppResult,
    models::{AppData, Settings},
};
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
        let config_path = path.join("config/config.json");
        let data_path = path.join("config/data.json");
        let icons_path = path.join("icons");
        // 锁文件独立于 JSON 文件。原子替换会更换 JSON 文件本身，
        // 因此不能把锁绑定在会被替换的目标文件句柄上。
        let config_lock_path = path.join("config/config.json.lock");
        let data_lock_path = path.join("config/data.json.lock");
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

    fn read_locked<T>(&self, file_path: &Path, lock_path: &Path) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.with_file_lock(lock_path, || Self::read_json(file_path))
    }

    fn write_locked<T>(&self, file_path: &Path, lock_path: &Path, value: &T) -> AppResult<()>
    where
        T: serde::Serialize,
    {
        self.with_file_lock(lock_path, || Self::write_json(file_path, value))
    }

    fn mutate_locked<T, Operation, R>(
        &self,
        file_path: &Path,
        lock_path: &Path,
        operation: Operation,
    ) -> AppResult<R>
    where
        T: serde::de::DeserializeOwned + serde::Serialize,
        Operation: FnOnce(&mut T) -> AppResult<R>,
    {
        self.with_file_lock(lock_path, || {
            mutate_snapshot(
                || Self::read_json(file_path),
                |value| Self::write_json(file_path, value),
                operation,
            )
        })
    }

    fn read_json<T>(path: &Path) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }
    fn write_json<T>(path: &Path, value: &T) -> AppResult<()>
    where
        T: serde::Serialize,
    {
        let content = serde_json::to_string_pretty(value)?;
        atomic_write(path, content.as_bytes())
    }
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
        self.read_locked(&self.data_path, &self.data_lock_path)
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
        self.mutate_locked(&self.data_path, &self.data_lock_path, operation)
    }
}

impl SettingsRepository for JsonStore {
    fn load_settings(&self) -> AppResult<Settings> {
        self.read_locked(&self.config_path, &self.config_lock_path)
    }
    fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.write_locked(&self.config_path, &self.config_lock_path, settings)
    }

    fn mutate_settings<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut Settings) -> AppResult<R>,
    {
        self.mutate_locked(&self.config_path, &self.config_lock_path, operation)
    }
}
