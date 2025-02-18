use std::{
    collections::HashMap,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::{
    sync::{watch, Semaphore, SemaphorePermit},
    task::JoinSet,
    time::sleep,
};

use crate::{
    config::Config,
    events::{DownloadEvent, DownloadTaskEvent},
    extensions::AnyhowErrorToStringChain,
    manhuagui_client::ManhuaguiClient,
    types::ChapterInfo,
};

/// 用于管理下载任务
///
/// 克隆 `DownloadManager` 的开销极小，性能开销几乎可以忽略不计。  
/// 可以放心地在多个线程中传递和使用它的克隆副本。  
///
/// 具体来说：  
/// - `app` 是 `AppHandle` 类型，根据 `Tauri` 文档，它的克隆开销是极小的。  
/// - 其他字段都被 `Arc` 包裹，这些字段的克隆操作仅仅是增加引用计数。  
#[derive(Clone)]
pub struct DownloadManager {
    app: AppHandle,
    chapter_sem: Arc<Semaphore>,
    img_sem: Arc<Semaphore>,
    byte_per_sec: Arc<AtomicU64>,
    download_tasks: Arc<RwLock<HashMap<i64, DownloadTask>>>,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let manager = DownloadManager {
            app: app.clone(),
            chapter_sem: Arc::new(Semaphore::new(1)),
            img_sem: Arc::new(Semaphore::new(10)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: Arc::new(RwLock::new(HashMap::new())),
        };

        tauri::async_runtime::spawn(Self::log_download_speed(app.clone()));

        manager
    }

    pub fn create_download_task(&self, chapter_info: ChapterInfo) {
        let task = DownloadTask::new(self.app.clone(), chapter_info);
        let chapter_id = task.chapter_info.chapter_id;
        tauri::async_runtime::spawn(task.clone().process());
        self.download_tasks.write().insert(chapter_id, task);
    }

    pub fn pause_download_task(&self, chapter_id: i64) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(anyhow!("未找到章节ID为`{chapter_id}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Paused);
        Ok(())
    }

    pub fn resume_download_task(&self, chapter_id: i64) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(anyhow!("未找到章节ID为`{chapter_id}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Pending);
        Ok(())
    }

    pub fn cancel_download_task(&self, chapter_id: i64) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&chapter_id) else {
            return Err(anyhow!("未找到章节ID为`{chapter_id}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Cancelled);
        Ok(())
    }

    #[allow(clippy::cast_precision_loss)]
    async fn log_download_speed(app: AppHandle) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let manager = app.state::<DownloadManager>();
            let byte_per_sec = manager.byte_per_sec.swap(0, Ordering::Relaxed);
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");
            // 发送总进度条下载速度事件
            let _ = DownloadEvent::Speed { speed }.emit(&app);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DownloadTaskState {
    Pending,
    Downloading,
    Paused,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Clone)]
struct DownloadTask {
    app: AppHandle,
    download_manager: DownloadManager,
    chapter_info: Arc<ChapterInfo>,
    state_sender: watch::Sender<DownloadTaskState>,
    downloaded_img_count: Arc<AtomicU32>,
    total_img_count: Arc<AtomicU32>,
}

impl DownloadTask {
    pub fn new(app: AppHandle, chapter_info: ChapterInfo) -> Self {
        let download_manager = app.state::<DownloadManager>().inner().clone();
        let (state_sender, _) = watch::channel(DownloadTaskState::Pending);
        Self {
            app,
            download_manager,
            chapter_info: Arc::new(chapter_info),
            state_sender,
            downloaded_img_count: Arc::new(AtomicU32::new(0)),
            total_img_count: Arc::new(AtomicU32::new(0)),
        }
    }

    async fn process(self) {
        let download_chapter_task = self.download_chapter();
        tokio::pin!(download_chapter_task);

        let mut state_receiver = self.state_sender.subscribe();
        state_receiver.mark_changed();
        let mut permit = None;
        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            let state_is_pending = *state_receiver.borrow() == DownloadTaskState::Pending;
            tokio::select! {
                () = &mut download_chapter_task, if state_is_downloading && permit.is_some() => break,
                control_flow = self.acquire_chapter_permit(&mut permit), if state_is_pending && permit.is_none() => {
                    match control_flow {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                },
                _ = state_receiver.changed() => {
                    match self.handle_state_change(&mut permit, &mut state_receiver) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                }
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    async fn download_chapter(&self) {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        let Some(img_urls) = self.get_img_urls().await else {
            return;
        };

        let Some(temp_download_dir) = self.create_temp_download_dir() else {
            return;
        };

        self.total_img_count
            .store(img_urls.len() as u32, Ordering::Relaxed);

        let mut join_set = JoinSet::new();
        for (i, url) in img_urls.into_iter().enumerate() {
            let save_path = temp_download_dir.join(format!("{:03}.jpg", i + 1));
            let url = url.clone();
            let download_img_task = DownloadImgTask::new(self, url, save_path);
            // 创建下载任务
            join_set.spawn(download_img_task.process());
        }
        // 等待所有下载任务完成
        join_set.join_all().await;
        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "所有图片下载任务完成"
        );
        // 检查此章节的图片是否全部下载成功
        let downloaded_img_count = self.downloaded_img_count.load(Ordering::Relaxed);
        let total_img_count = self.total_img_count.load(Ordering::Relaxed);
        // 此章节的图片未全部下载成功
        if downloaded_img_count != total_img_count {
            let err_title = format!("`{comic_title} - {group_name} - {chapter_title}`下载不完整");
            let err_msg =
                format!("总共有`{total_img_count}`张图片，但只下载了`{downloaded_img_count}`张");
            tracing::error!(err_title, message = err_msg);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_event();

            return;
        }
        if let Err(err) = self.rename_temp_download_dir(&temp_download_dir) {
            let err_title =
                format!("`{comic_title} - {group_name} - {chapter_title}`重命名临时目录失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_event();

            return;
        }
        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "重命名临时下载目录`{temp_download_dir:?}`成功"
        );
        // 章节下载成功
        tracing::info!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "章节下载成功"
        );
        // 每个章节下载完成后，等待一段时间
        self.sleep_between_chapters().await;
        // 发送下载结束事件
        self.set_state(DownloadTaskState::Completed);
        self.emit_download_task_event();
    }

    async fn acquire_chapter_permit<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
    ) -> ControlFlow<()> {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        tracing::debug!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "章节开始排队"
        );

        self.emit_download_task_event();

        *permit = match permit.take() {
            // 如果有permit，则直接用
            Some(permit) => Some(permit),
            // 如果没有permit，则获取permit
            None => match self
                .download_manager
                .chapter_sem
                .acquire()
                .await
                .map_err(anyhow::Error::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title = format!(
                        "`{comic_title} - {group_name} - {chapter_title}`获取下载章节的permit失败"
                    );
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);

                    self.set_state(DownloadTaskState::Failed);
                    self.emit_download_task_event();

                    return ControlFlow::Break(());
                }
            },
        };
        // 将任务状态设置为`Downloading`
        if let Err(err) = self
            .state_sender
            .send(DownloadTaskState::Downloading)
            .map_err(anyhow::Error::from)
        {
            let err_title = format!(
                "`{comic_title} - {group_name} - {chapter_title}`发送状态`Downloading`失败"
            );
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) -> ControlFlow<()> {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        self.emit_download_task_event();
        let state = *state_receiver.borrow();
        match state {
            DownloadTaskState::Paused => {
                tracing::debug!(
                    chapter_id,
                    comic_title,
                    group_name,
                    chapter_title,
                    "章节暂停中"
                );
                if let Some(permit) = permit.take() {
                    drop(permit);
                };
                ControlFlow::Continue(())
            }
            DownloadTaskState::Cancelled => {
                tracing::debug!(
                    chapter_id,
                    comic_title,
                    group_name,
                    chapter_title,
                    "章节取消下载"
                );
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    }

    async fn get_img_urls(&self) -> Option<Vec<String>> {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "章节开始获取图片链接"
        );
        let img_urls = match self
            .manhuagui_client()
            .get_img_urls(&self.chapter_info)
            .await
        {
            Ok(urls) => urls,
            Err(err) => {
                let err_title =
                    format!("`{comic_title} - {group_name} - {chapter_title}`获取图片链接失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);

                self.set_state(DownloadTaskState::Failed);
                self.emit_download_task_event();

                return None;
            }
        };
        Some(img_urls)
    }

    fn create_temp_download_dir(&self) -> Option<PathBuf> {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;
        let prefixed_chapter_title = &self.chapter_info.prefixed_chapter_title;

        let temp_download_dir = self
            .app
            .state::<RwLock<Config>>()
            .read()
            .download_dir
            .join(comic_title)
            .join(group_name)
            .join(format!(".下载中-{prefixed_chapter_title}")); // 以 `.下载中-` 开头，表示是临时目录

        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(anyhow::Error::from) {
            let err_title = format!("`{comic_title} - {group_name} - {chapter_title}`创建目录`{temp_download_dir:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_event();

            return None;
        }
        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "创建临时下载目录`{temp_download_dir:?}`成功",
        );
        Some(temp_download_dir)
    }

    fn rename_temp_download_dir(&self, temp_download_dir: &Path) -> anyhow::Result<()> {
        let Some(parent) = temp_download_dir.parent() else {
            return Err(anyhow!("无法获取`{temp_download_dir:?}`的父目录"));
        };

        let download_dir = parent.join(&self.chapter_info.prefixed_chapter_title);

        if download_dir.exists() {
            std::fs::remove_dir_all(&download_dir)
                .context(format!("删除目录`{download_dir:?}`失败"))?;
        }

        std::fs::rename(temp_download_dir, &download_dir).context(format!(
            "将`{temp_download_dir:?}`重命名为`{download_dir:?}`失败"
        ))?;

        Ok(())
    }

    async fn sleep_between_chapters(&self) {
        let chapter_id = self.chapter_info.chapter_id;
        let mut remaining_sec = self
            .app
            .state::<RwLock<Config>>()
            .read()
            .download_interval_sec;
        while remaining_sec > 0 {
            // 发送章节休眠事件
            let _ = DownloadEvent::Sleeping {
                chapter_id,
                remaining_sec,
            }
            .emit(&self.app);
            sleep(Duration::from_secs(1)).await;
            remaining_sec -= 1;
        }
    }

    fn set_state(&self, state: DownloadTaskState) {
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;
        if let Err(err) = self.state_sender.send(state).map_err(anyhow::Error::from) {
            let err_title =
                format!("`{comic_title} - {group_name} - {chapter_title}`发送状态`{state:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }
    }

    fn emit_download_task_event(&self) {
        let _ = DownloadTaskEvent {
            state: *self.state_sender.borrow(),
            chapter_info: self.chapter_info.as_ref().clone(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }
    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.state::<ManhuaguiClient>().inner().clone()
    }
}

#[derive(Clone)]
struct DownloadImgTask {
    app: AppHandle,
    download_manager: DownloadManager,
    chapter_info: Arc<ChapterInfo>,
    state_sender: watch::Sender<DownloadTaskState>,
    downloaded_img_count: Arc<AtomicU32>,
    total_img_count: Arc<AtomicU32>,
    url: String,
    save_path: PathBuf,
}

impl DownloadImgTask {
    pub fn new(download_task: &DownloadTask, url: String, save_path: PathBuf) -> Self {
        Self {
            app: download_task.app.clone(),
            download_manager: download_task.download_manager.clone(),
            chapter_info: download_task.chapter_info.clone(),
            state_sender: download_task.state_sender.clone(),
            downloaded_img_count: download_task.downloaded_img_count.clone(),
            total_img_count: download_task.total_img_count.clone(),
            url,
            save_path,
        }
    }

    async fn process(self) {
        let download_img_task = self.download_img();
        tokio::pin!(download_img_task);

        let mut state_receiver = self.state_sender.subscribe();
        state_receiver.mark_changed();
        let mut permit = None;

        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            tokio::select! {
                () = &mut download_img_task, if state_is_downloading && permit.is_some() => break,
                control_flow = self.acquire_img_permit(&mut permit), if state_is_downloading && permit.is_none() => {
                    match control_flow {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                },
                _ = state_receiver.changed() => {
                    match self.handle_state_change(&mut permit, &mut state_receiver) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                }
            }
        }
    }

    async fn download_img(&self) {
        let url = &self.url;
        let save_path = &self.save_path;
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            url,
            "开始下载图片"
        );

        let img_data = match self.manhuagui_client().get_img_bytes(url).await {
            Ok(data) => data,
            Err(err) => {
                let err_title = format!("下载图片`{url}`失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return;
            }
        };

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            url,
            "图片成功下载到内存"
        );
        // 保存图片
        if let Err(err) = std::fs::write(save_path, &img_data).map_err(anyhow::Error::from) {
            let err_title = format!("保存图片`{save_path:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }
        tracing::trace!(chapter_id, url, "图片成功保存到`{save_path:?}`");
        // 记录下载字节数
        self.download_manager
            .byte_per_sec
            .fetch_add(img_data.len() as u64, Ordering::Relaxed);
        tracing::debug!(chapter_id, url, "图片下载成功");

        let _ = DownloadTaskEvent {
            state: *self.state_sender.borrow(),
            chapter_info: self.chapter_info.as_ref().clone(),
            downloaded_img_count: self.downloaded_img_count.fetch_add(1, Ordering::Relaxed) + 1,
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    async fn acquire_img_permit<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
    ) -> ControlFlow<()> {
        let url = &self.url;
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            url,
            "图片开始排队"
        );

        *permit = match permit.take() {
            // 如果有permit，则直接用
            Some(permit) => Some(permit),
            // 如果没有permit，则获取permit
            None => match self
                .download_manager
                .img_sem
                .acquire()
                .await
                .map_err(anyhow::Error::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title = format!(
                        "`{comic_title} - {group_name} - {chapter_title}`获取下载图片的permit失败"
                    );
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    return ControlFlow::Break(());
                }
            },
        };
        ControlFlow::Continue(())
    }

    fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) -> ControlFlow<()> {
        let url = &self.url;
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        let state = *state_receiver.borrow();
        match state {
            DownloadTaskState::Paused => {
                tracing::trace!(
                    chapter_id,
                    comic_title,
                    group_name,
                    chapter_title,
                    url,
                    "图片暂停下载"
                );
                if let Some(permit) = permit.take() {
                    drop(permit);
                };
                ControlFlow::Continue(())
            }
            DownloadTaskState::Cancelled => {
                tracing::trace!(
                    chapter_id,
                    comic_title,
                    group_name,
                    chapter_title,
                    url,
                    "图片取消下载"
                );
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    }

    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.state::<ManhuaguiClient>().inner().clone()
    }
}
