use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{DataRepository, SettingsRepository};
use crate::{
    error::AppResult,
    models::{AppData, Settings},
};

#[derive(Debug, Clone)]
pub struct JsonStore {
    path: PathBuf,
    config_path: PathBuf,
    data_path: PathBuf,
    icons_path: PathBuf,
}

impl JsonStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let config_path = path.join("config/config.json");
        let data_path = path.join("config/data.json");
        let icons_path = path.join("icons");
        Self {
            path,
            config_path,
            data_path,
            icons_path,
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
        if !self.data_path.exists() {
            self.save_data(&AppData::default())?;
        }
        if !self.config_path.exists() {
            self.save_settings(&Settings::default())?;
        }
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

    pub(crate) fn icons_path(&self) -> &Path {
        &self.icons_path
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
    let write_result = (|| -> AppResult<()> {
        let mut file = File::create(&temp_path)?;
        file.write_all(content)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);

        replace_file(&temp_path, path)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn temporary_path(path: &Path) -> AppResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| crate::models::AppError::new("json_name_invalid", "JSON 文件名无效"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| crate::models::AppError::new("json_clock_error", error.to_string()))?
        .as_nanos();

    Ok(path.with_file_name(format!(".{file_name}.{timestamp}.tmp")))
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
        Self::read_json(self.data_path.as_path())
    }
    fn save_data(&self, value: &AppData) -> AppResult<()> {
        Self::write_json(self.data_path.as_path(), value)
    }
}

impl SettingsRepository for JsonStore {
    fn load_settings(&self) -> AppResult<Settings> {
        Self::read_json(self.config_path.as_path())
    }
    fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        Self::write_json(self.config_path.as_path(), settings)
    }
}
