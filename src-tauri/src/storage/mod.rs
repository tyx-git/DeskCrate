use crate::{
    error::AppResult,
    models::{AppData, Settings},
};

pub mod json;

pub use json::JsonStore;

// 对一个持久化快照执行“读取、修改、保存”流程。
//
// 锁的获取由具体仓储控制：JsonStore会在调用此函数前持有跨进程锁，
// 内存仓储则可以直接复用这个流程而无需文件锁。
pub(crate) fn mutate_snapshot<T, Load, Save, Operation, R>(
    load: Load,
    save: Save,
    operation: Operation,
) -> AppResult<R>
where
    Load: FnOnce() -> AppResult<T>,
    Save: FnOnce(&T) -> AppResult<()>,
    Operation: FnOnce(&mut T) -> AppResult<R>,
{
    let mut snapshot = load()?;
    let result = operation(&mut snapshot)?;
    save(&snapshot)?;
    Ok(result)
}

// 持久化目录数据的最小抽象。
// service 层只依赖这个 trait，因此业务规则可以使用内存仓储测试，
// 不需要直接操作真实文件系统。
pub trait DataRepository: Send + Sync {
    fn load_data(&self) -> AppResult<AppData>;
    fn save_data(&self, data: &AppData) -> AppResult<()>;

    // 在内存中的数据快照上执行一次业务修改，并在成功后持久化。
    //
    // 这个默认实现适用于不需要跨进程协调的仓储；共享文件仓储必须覆盖
    // 此方法，把读取、修改和写回放进同一个跨进程锁内。JsonStore 已覆盖。
    fn mutate_data<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut AppData) -> AppResult<R>,
    {
        mutate_snapshot(|| self.load_data(), |data| self.save_data(data), operation)
    }
}
pub trait SettingsRepository: Send + Sync {
    fn load_settings(&self) -> AppResult<Settings>;
    fn save_settings(&self, settings: &Settings) -> AppResult<()>;

    // 在内存中的配置快照上执行一次业务修改，并在成功后持久化。
    // 共享文件仓储必须覆盖此方法，让完整读改写过程处在跨进程锁内。
    fn mutate_settings<F, R>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(&mut Settings) -> AppResult<R>,
    {
        mutate_snapshot(
            || self.load_settings(),
            |settings| self.save_settings(settings),
            operation,
        )
    }
}
