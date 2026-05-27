use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use eyre::{eyre, WrapErr};
use parking_lot::RwLock;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::Semaphore;
use tracing::instrument;

use crate::{
    downloader::{download_task::DownloadTask, download_task_state::DownloadTaskState},
    events::DownloadEvent,
    extensions::AppHandleExt,
    types::Comic,
};

pub struct DownloadManager {
    pub app: AppHandle,
    pub chapter_sem: Arc<Semaphore>,
    pub img_sem: Arc<Semaphore>,
    pub byte_per_sec: Arc<AtomicU64>,
    download_tasks: RwLock<HashMap<i64, Arc<DownloadTask>>>,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let (chapter_concurrency, img_concurrency) = {
            let config = app.get_config();
            let config = config.read();
            (config.chapter_concurrency, config.img_concurrency)
        };

        let manager = DownloadManager {
            app: app.clone(),
            chapter_sem: Arc::new(Semaphore::new(chapter_concurrency)),
            img_sem: Arc::new(Semaphore::new(img_concurrency)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: RwLock::new(HashMap::new()),
        };

        tauri::async_runtime::spawn(Self::emit_download_speed_loop(
            manager.app.clone(),
            manager.byte_per_sec.clone(),
        ));

        manager
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(comic_id = comic.id, comic_title = comic.title, chapter_id = chapter_id)
    )]
    pub fn create_download_task(&self, comic: Comic, chapter_id: i64) -> eyre::Result<()> {
        use DownloadTaskState::{Downloading, Paused, Pending};

        let mut tasks = self.download_tasks.write();

        if let Some(task) = tasks.get(&chapter_id) {
            let state = *task.state_sender.borrow();
            if matches!(state, Pending | Downloading | Paused) {
                return Err(eyre!("章节ID对应的下载任务已存在"));
            }
        }

        if let Some(task) = tasks.remove(&chapter_id) {
            task.delete_sender
                .send(())
                .wrap_err("通知旧下载任务删除失败")?;
        }

        let task = DownloadTask::new(self.app.clone(), comic, chapter_id)?;
        tasks.insert(chapter_id, task);
        Ok(())
    }

    #[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
    pub fn pause_download_task(&self, chapter_id: i64) -> eyre::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(eyre!("未找到章节ID对应的下载任务"));
        };
        task.set_state(DownloadTaskState::Paused);
        Ok(())
    }

    #[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
    pub fn resume_download_task(&self, chapter_id: i64) -> eyre::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(eyre!("未找到章节ID对应的下载任务"));
        };
        task.set_state(DownloadTaskState::Pending);
        Ok(())
    }

    #[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
    pub fn delete_download_task(&self, chapter_id: i64) -> eyre::Result<()> {
        let mut tasks = self.download_tasks.write();
        let Some(task) = tasks.remove(&chapter_id) else {
            return Err(eyre!("未找到章节ID对应的下载任务"));
        };
        task.delete_sender
            .send(())
            .wrap_err("通知下载任务删除失败")?;
        Ok(())
    }

    async fn emit_download_speed_loop(app: AppHandle, byte_per_sec: Arc<AtomicU64>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let byte_per_sec = byte_per_sec.swap(0, Ordering::Relaxed);
            #[allow(clippy::cast_precision_loss)]
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");
            let _ = DownloadEvent::Speed { speed }.emit(&app);
        }
    }
}
