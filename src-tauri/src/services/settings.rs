use crate::{
    error::AppResult,
    models::{AppError, Settings},
    state::AppState,
    storage::SettingsRepository,
};

// 读取 `config.json` 中的全部应用配置。
pub fn get_settings(state: &AppState) -> AppResult<Settings> {
    let store = state.get_lock()?;
    store.load_settings()
}

// 校验并持久化应用配置。
//
// 当前配置主要由窗口层使用，因此先保证保存的窗口宽高是有效的；
// 位置允许为负数，以兼容多显示器和主显示器左侧的屏幕。
pub fn update_settings(state: &AppState, settings: Settings) -> AppResult<Settings> {
    validate_settings(&settings)?;

    let store = state.get_lock()?;
    store.mutate_settings(|current| {
        *current = settings.clone();
        Ok(())
    })?;
    Ok(settings)
}

fn validate_settings(settings: &Settings) -> AppResult<()> {
    if let Some(size) = settings.window.size {
        if size.width == 0 || size.height == 0 {
            return Err(AppError::new(
                "invalid_window_size",
                "窗口宽度和高度必须大于 0",
            ));
        }
    }

    // `is_initialized` 与几何信息必须成套存在。否则下次启动会进入
    // “恢复窗口”分支，却没有可恢复的位置或大小。
    if settings.window.is_initialized {
        if settings.window.position.is_none() {
            return Err(AppError::new(
                "invalid_window_state",
                "已初始化配置必须包含窗口位置",
            ));
        }

        if settings.window.size.is_none() {
            return Err(AppError::new(
                "invalid_window_state",
                "已初始化配置必须包含窗口大小",
            ));
        }
    }

    Ok(())
}
