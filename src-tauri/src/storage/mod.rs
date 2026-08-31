use crate::{
    error::AppResult,
    models::{AppData, Settings},
};

pub mod json;

pub use json::JsonStore;

// 持久化目录数据的最小抽象。
// service 层只依赖这个 trait，因此业务规则可以使用内存仓储测试，
// 不需要直接操作真实文件系统。
pub trait DataRepository: Send + Sync {
    fn load_data(&self) -> AppResult<AppData>;
    fn save_data(&self, data: &AppData) -> AppResult<()>;

    // 在内存中的数据快照上执行一次业务修改，并在成功后持久化。
    //
    // 具体存储实现可以保证“修改失败不落盘”，而 service 层只需要
    // 描述如何修改 `AppData`，不需要重复编排读取和保存流程。
    fn mutate_data<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut AppData) -> AppResult<R>,
    {
        let mut data = self.load_data()?;
        let result = operation(&mut data)?;
        self.save_data(&data)?;
        Ok(result)
    }
}
pub trait SettingsRepository: Send + Sync {
    fn load_settings(&self) -> AppResult<Settings>;
    fn save_settings(&self, settings: &Settings) -> AppResult<()>;

    // 在内存中的配置快照上执行一次业务修改，并在成功后持久化。
    fn mutate_settings<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut Settings) -> AppResult<R>,
    {
        let mut settings = self.load_settings()?;
        let result = operation(&mut settings)?;
        self.save_settings(&settings)?;
        Ok(result)
    }
}
