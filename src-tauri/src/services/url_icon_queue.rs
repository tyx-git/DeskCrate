use std::sync::{
    mpsc::{self, SyncSender, TrySendError},
    Arc, Mutex, OnceLock,
};
use std::thread;

use tauri::{AppHandle, Emitter};

use crate::{
    commands::catalog::ICON_READY_EVENT, services::catalog, state::AppState, storage::JsonStore,
};

const WORKER_COUNT: usize = 4;
const QUEUE_CAPACITY: usize = 64;

struct UrlIconJob {
    store: JsonStore,
    app: AppHandle,
    app_id: String,
}

struct WorkerPool<T> {
    sender: SyncSender<T>,
}

impl<T: Send + 'static> WorkerPool<T> {
    fn new(
        worker_count: usize,
        queue_capacity: usize,
        handler: impl Fn(T) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let handler = Arc::new(handler);

        for index in 0..worker_count {
            let receiver = Arc::clone(&receiver);
            let handler = Arc::clone(&handler);
            thread::Builder::new()
                .name(format!("deskcrate-icon-{index}"))
                .spawn(move || loop {
                    // Release the receiver lock immediately after dequeueing so the other
                    // workers can independently process jobs while this one does I/O.
                    let job = {
                        let Ok(receiver) = receiver.lock() else {
                            return;
                        };
                        receiver.recv()
                    };
                    match job {
                        Ok(job) => handler(job),
                        Err(_) => return,
                    }
                })?;
        }

        Ok(Self { sender })
    }

    fn try_send(&self, job: T) -> Result<(), TrySendError<T>> {
        self.sender.try_send(job)
    }
}

static QUEUE: OnceLock<Result<WorkerPool<UrlIconJob>, String>> = OnceLock::new();

/// Enqueue a best-effort icon refresh. Catalog mutations have already committed by this point,
/// so a full queue or worker startup failure must not make the add command report a false failure.
pub(crate) fn enqueue(state: &AppState, app: &AppHandle, app_id: String) {
    let queue = QUEUE.get_or_init(|| {
        WorkerPool::new(WORKER_COUNT, QUEUE_CAPACITY, process_job)
            .map_err(|error| format!("无法启动网址图标 worker: {error}"))
    });
    let queue = match queue {
        Ok(queue) => queue,
        Err(error) => {
            log::warn!("{error}；{app_id} 保留内置图标");
            return;
        }
    };

    let store = match state.get_lock() {
        Ok(store) => store.clone(),
        Err(error) => {
            log::warn!("无法为 {app_id} 排队补抓图标: {}", error.message);
            return;
        }
    };
    let job = UrlIconJob {
        store,
        app: app.clone(),
        app_id,
    };
    if let Err(error) = queue.try_send(job) {
        match error {
            TrySendError::Full(job) => {
                log::warn!("网址图标任务队列已满，{} 保留内置图标", job.app_id);
            }
            TrySendError::Disconnected(job) => {
                log::warn!("网址图标 worker 已退出，{} 保留内置图标", job.app_id);
            }
        }
    }
}

fn process_job(job: UrlIconJob) {
    let state = AppState::new(job.store);
    match catalog::refresh_url_icon(&state, &job.app_id) {
        Ok(Some(icon_url)) => {
            if let Err(error) = job.app.emit(
                ICON_READY_EVENT,
                serde_json::json!({ "appId": job.app_id, "iconUrl": icon_url }),
            ) {
                log::warn!("通知图标就绪失败: {error}");
            }
        }
        Ok(None) => log::debug!("{} 没有可补的网址图标，继续用内置图标", job.app_id),
        Err(error) => log::debug!("补抓 {} 的网址图标失败: {}", job.app_id, error.message),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc, Arc, Barrier,
        },
        time::Duration,
    };

    use super::*;

    #[test]
    fn pool_runs_jobs_with_at_most_four_workers() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let first_batch = Arc::new(Barrier::new(WORKER_COUNT));
        let (done_tx, done_rx) = mpsc::channel();

        let worker_active = Arc::clone(&active);
        let worker_peak = Arc::clone(&peak);
        let worker_barrier = Arc::clone(&first_batch);
        let pool = WorkerPool::new(WORKER_COUNT, 32, move |job: usize| {
            let running = worker_active.fetch_add(1, Ordering::SeqCst) + 1;
            worker_peak.fetch_max(running, Ordering::SeqCst);
            if job < WORKER_COUNT {
                worker_barrier.wait();
            } else {
                thread::sleep(Duration::from_millis(2));
            }
            worker_active.fetch_sub(1, Ordering::SeqCst);
            done_tx.send(()).unwrap();
        })
        .unwrap();

        for job in 0..24 {
            pool.try_send(job).unwrap();
        }
        for _ in 0..24 {
            done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), WORKER_COUNT);
    }

    #[test]
    fn bounded_queue_reports_full_without_blocking() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let worker_release = Arc::clone(&release_rx);
        let pool = WorkerPool::new(1, 1, move |_: usize| {
            worker_release.lock().unwrap().recv().unwrap();
        })
        .unwrap();

        pool.try_send(0).unwrap();
        // Queue one pending item, then verify a third item is rejected while the worker is busy.
        for _ in 0..100 {
            if pool.try_send(1).is_ok() {
                break;
            }
            thread::yield_now();
        }
        assert!(matches!(pool.try_send(2), Err(TrySendError::Full(2))));
        release_tx.send(()).unwrap();
    }
}
