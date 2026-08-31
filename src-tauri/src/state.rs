use std::sync::{Mutex, MutexGuard};

use crate::{error::AppResult, models::AppError, storage::JsonStore};

pub struct AppState {
    store: Mutex<JsonStore>, // 私有成员，只能通过get_lock函数获取锁
}

impl AppState {
    pub fn new(store: JsonStore) -> Self {
        Self {
            store: Mutex::new(store),
        }
    }
    pub(crate) fn get_lock(&self) -> AppResult<MutexGuard<'_, JsonStore>> {
        self.store
            .lock()
            .map_err(|_| AppError::new("get_lock_failed", "无法获取内存锁"))
    }
}
