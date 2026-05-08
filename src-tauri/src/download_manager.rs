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
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::{
    sync::{watch, Semaphore, SemaphorePermit},
    task::JoinSet,
    time::sleep,
};

use crate::{
    events::{DownloadSleepingEvent, DownloadSpeedEvent, DownloadTaskEvent},
    extensions::{AnyhowErrorToStringChain, AppHandleExt},
    manhuagui_client::ManhuaguiClient,
    types::{ChapterInfo, Comic},
    utils,
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
            download_tasks: Arc::new(RwLock::new(HashMap::new())),
        };

        tauri::async_runtime::spawn(Self::log_download_speed(app.clone()));

        manager
    }

    pub fn create_download_task(&self, comic: Comic, chapter_id: i64) -> anyhow::Result<()> {
        use DownloadTaskState::{Downloading, Paused, Pending};

        let mut tasks = self.download_tasks.write();
        if let Some(task) = tasks.get(&chapter_id) {
            // 如果任务已经存在，且状态是`Pending`、`Downloading`或`Paused`，则不创建新任务
            let state = *task.state_sender.borrow();
            if matches!(state, Pending | Downloading | Paused) {
                return Err(anyhow!("章节ID为`{chapter_id}`的下载任务已存在"));
            }
        }
        tasks.remove(&chapter_id);
        let task = DownloadTask::new(self.app.clone(), comic, chapter_id)
            .context("DownloadTask创建失败")?;
        tauri::async_runtime::spawn(task.clone().process());
        tasks.insert(chapter_id, task);
        Ok(())
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
            let manager = app.get_download_manager();
            let byte_per_sec = manager.byte_per_sec.swap(0, Ordering::Relaxed);
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");
            // 发送总进度条下载速度事件
            let _ = DownloadSpeedEvent { speed }.emit(&app);
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
    comic: Arc<Comic>,
    chapter_info: Arc<ChapterInfo>,
    state_sender: watch::Sender<DownloadTaskState>,
    downloaded_img_count: Arc<AtomicU32>,
    total_img_count: Arc<AtomicU32>,
}

impl DownloadTask {
    pub fn new(app: AppHandle, mut comic: Comic, chapter_id: i64) -> anyhow::Result<Self> {
        comic
            .update_download_dir_fields_by_fmt(&app)
            .context(format!("漫画`{}`更新`download_dir`字段失败", comic.title))?;

        let chapter_info = comic
            .groups
            .iter()
            .flat_map(|(_, chapter_infos)| chapter_infos.iter())
            .find(|chapter_info| chapter_info.chapter_id == chapter_id)
            .cloned()
            .context(format!("未找到章节ID为`{chapter_id}`的章节信息"))?;

        let download_manager = app.get_download_manager().inner().clone();
        let (state_sender, _) = watch::channel(DownloadTaskState::Pending);

        let task = Self {
            app,
            download_manager,
            comic: Arc::new(comic),
            chapter_info: Arc::new(chapter_info),
            state_sender,
            downloaded_img_count: Arc::new(AtomicU32::new(0)),
            total_img_count: Arc::new(AtomicU32::new(0)),
        };

        Ok(task)
    }

    async fn process(self) {
        self.emit_download_task_create_event();

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
                control_flow = self.acquire_chapter_permit(&mut permit), if state_is_pending => {
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

    async fn download_chapter(&self) {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        if let Err(err) = self.comic.save_metadata() {
            let err_title = format!("`{comic_title}`保存元数据失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }

        let Some(img_urls) = self.get_img_urls().await else {
            return;
        };

        #[allow(clippy::cast_possible_truncation)]
        self.total_img_count
            .store(img_urls.len() as u32, Ordering::Relaxed);

        let Some(temp_download_dir) = self.create_temp_download_dir() else {
            return;
        };

        let mut join_set = JoinSet::new();
        for (i, url) in img_urls.into_iter().enumerate() {
            let url = url.clone();
            let temp_download_dir = temp_download_dir.clone();
            // 创建下载任务
            let download_img_task = DownloadImgTask::new(self, url, i, temp_download_dir);
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
            self.emit_download_task_update_event();

            return;
        }

        if let Err(err) = self.rename_temp_download_dir(&temp_download_dir) {
            let err_title =
                format!("`{comic_title} - {group_name} - {chapter_title}`重命名临时目录失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }

        if let Err(err) = self.chapter_info.save_metadata() {
            let err_title = format!("`{comic_title} - {chapter_title}`保存章节元数据失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }

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
        self.emit_download_task_update_event();
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

        self.emit_download_task_update_event();

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
                    self.emit_download_task_update_event();

                    return ControlFlow::Break(());
                }
            },
        };
        // 如果当前任务状态不是`Pending`，则不将任务状态设置为`Downloading`
        if *self.state_sender.borrow() != DownloadTaskState::Pending {
            return ControlFlow::Continue(());
        }
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

        self.emit_download_task_update_event();
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
                }
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
                self.emit_download_task_update_event();

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

        let temp_download_dir = match self.chapter_info.get_temp_download_dir() {
            Ok(temp_download_dir) => temp_download_dir,
            Err(err) => {
                let err_title =
                    format!("`{comic_title} - {group_name} - {chapter_title}`获取临时下载目录失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);

                self.set_state(DownloadTaskState::Failed);
                self.emit_download_task_update_event();

                return None;
            }
        };

        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(anyhow::Error::from) {
            let err_title = format!(
                "`{comic_title} - {group_name} - {chapter_title}`创建目录`{}`失败",
                temp_download_dir.display()
            );
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return None;
        }

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "创建临时下载目录`{}`成功",
            temp_download_dir.display()
        );

        Some(temp_download_dir)
    }

    fn rename_temp_download_dir(&self, temp_download_dir: &PathBuf) -> anyhow::Result<()> {
        let chapter_download_dir = self
            .chapter_info
            .chapter_download_dir
            .as_ref()
            .context("`chapter_download_dir`字段为`None`")?;

        if chapter_download_dir.exists() {
            std::fs::remove_dir_all(chapter_download_dir)
                .context(format!("删除`{}`失败", chapter_download_dir.display()))?;
        }

        std::fs::rename(temp_download_dir, chapter_download_dir).context(format!(
            "将`{}`重命名为`{}`失败",
            temp_download_dir.display(),
            chapter_download_dir.display()
        ))?;

        Ok(())
    }

    async fn sleep_between_chapters(&self) {
        let chapter_id = self.chapter_info.chapter_id;
        let mut remaining_sec = self.app.get_config().read().chapter_download_interval_sec;
        while remaining_sec > 0 {
            // 发送章节休眠事件
            let _ = DownloadSleepingEvent {
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

    fn emit_download_task_update_event(&self) {
        let _ = DownloadTaskEvent::Update {
            chapter_id: self.chapter_info.chapter_id,
            state: *self.state_sender.borrow(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn emit_download_task_create_event(&self) {
        let _ = DownloadTaskEvent::Create {
            state: *self.state_sender.borrow(),
            comic: self.comic.as_ref().clone(),
            chapter_info: self.chapter_info.as_ref().clone(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }
    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.get_manhuagui_client().inner().clone()
    }
}

#[derive(Clone)]
struct DownloadImgTask {
    app: AppHandle,
    download_manager: DownloadManager,
    download_task: DownloadTask,
    chapter_info: Arc<ChapterInfo>,
    url: String,
    index: usize,
    temp_download_dir: PathBuf,
}

impl DownloadImgTask {
    pub fn new(
        download_task: &DownloadTask,
        url: String,
        index: usize,
        temp_download_dir: PathBuf,
    ) -> Self {
        Self {
            app: download_task.app.clone(),
            download_manager: download_task.download_manager.clone(),
            download_task: download_task.clone(),
            chapter_info: download_task.chapter_info.clone(),
            url,
            index,
            temp_download_dir,
        }
    }

    async fn process(self) {
        let download_img_task = self.download_img();
        tokio::pin!(download_img_task);

        let mut state_receiver = self.download_task.state_sender.subscribe();
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
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        let save_path = self
            .temp_download_dir
            .join(format!("{:03}.jpg", self.index + 1));
        if save_path.exists() {
            // 如果图片已经存在，则直接跳过下载
            self.download_task
                .downloaded_img_count
                .fetch_add(1, Ordering::Relaxed);

            self.download_task.emit_download_task_update_event();

            tracing::trace!(
                chapter_id,
                comic_title,
                group_name,
                chapter_title,
                url,
                "图片已存在，跳过下载"
            );
            return;
        }

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
        if let Err(err) = std::fs::write(&save_path, &img_data).map_err(anyhow::Error::from) {
            let err_title = format!("保存图片`{}`失败", save_path.display());
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }
        tracing::trace!(chapter_id, url, "图片成功保存到`{}`", save_path.display());
        // 记录下载字节数
        self.download_manager
            .byte_per_sec
            .fetch_add(img_data.len() as u64, Ordering::Relaxed);

        self.download_task
            .downloaded_img_count
            .fetch_add(1, Ordering::Relaxed);

        self.download_task.emit_download_task_update_event();

        let img_download_interval_sec = self.app.get_config().read().img_download_interval_sec;
        sleep(Duration::from_secs(img_download_interval_sec)).await;
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
                }
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
        self.app.get_manhuagui_client().inner().clone()
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize, Type)]
pub struct ComicDirFmtParams {
    pub comic_id: i64,
    pub comic_title: String,
    pub comic_subtitle: String,
    pub pub_year: i64,
    pub region: String,
    pub author: String,
}

impl Comic {
    /// 根据fmt更新`comic_download_dir`和`chapter_infos.chapter_download_dir`字段
    fn update_download_dir_fields_by_fmt(&mut self, app: &AppHandle) -> anyhow::Result<()> {
        let comic_id = self.id;
        let comic_title = self.title.clone();
        let comic_subtitle = self.subtitle.clone().unwrap_or_default();
        let author = self.authors.join(", ");

        let comic_dir_fmt_params = ComicDirFmtParams {
            comic_id,
            comic_title: comic_title.clone(),
            comic_subtitle: comic_subtitle.clone(),
            pub_year: self.year,
            region: self.region.clone(),
            author: author.clone(),
        };
        let comic_download_dir = Comic::get_comic_download_dir_by_fmt(app, &comic_dir_fmt_params)?;
        self.comic_download_dir = Some(comic_download_dir.clone());

        for chapter_info in &mut self.groups.iter_mut().flat_map(|(_, chapters)| chapters) {
            let chapter_dir_fmt_params = ChapterDirFmtParams {
                comic_id,
                comic_title: comic_title.clone(),
                comic_subtitle: comic_subtitle.clone(),
                pub_year: self.year,
                region: self.region.clone(),
                author: author.clone(),
                group_name: chapter_info.group_name.clone(),
                chapter_id: chapter_info.chapter_id,
                chapter_title: chapter_info.chapter_title.clone(),
                order: chapter_info.order,
            };
            let chapter_download_dir = ChapterInfo::get_chapter_download_dir_by_fmt(
                app,
                &comic_download_dir,
                &chapter_dir_fmt_params,
            )?;
            chapter_info.chapter_download_dir = Some(chapter_download_dir);
        }

        Ok(())
    }

    fn get_comic_download_dir_by_fmt(
        app: &AppHandle,
        fmt_params: &ComicDirFmtParams,
    ) -> anyhow::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .context("将ComicDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .context("ComicDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .into_iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();

        let (download_dir, comic_dir_fmt) = {
            let config = app.get_config();
            let config = config.read();
            (config.download_dir.clone(), config.comic_dir_fmt.clone())
        };

        let dir_fmt_parts: Vec<&str> = comic_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).context("格式化目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }
        // 将格式化后的目录名拼接成完整的目录路径
        let mut comic_download_dir = download_dir;
        for dir_name in dir_names {
            comic_download_dir = comic_download_dir.join(dir_name);
        }

        Ok(comic_download_dir)
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize, Type)]
pub struct ChapterDirFmtParams {
    pub comic_id: i64,
    pub comic_title: String,
    pub comic_subtitle: String,
    pub pub_year: i64,
    pub region: String,
    pub author: String,
    pub group_name: String,
    pub chapter_id: i64,
    pub chapter_title: String,
    pub order: f64,
}

impl ChapterInfo {
    fn get_chapter_download_dir_by_fmt(
        app: &AppHandle,
        comic_download_dir: &Path,
        fmt_params: &ChapterDirFmtParams,
    ) -> anyhow::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .context("将ChapterDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .context("ChapterDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .into_iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();
        let mut chapter_dir_fmt = app.get_config().read().chapter_dir_fmt.clone();
        Self::preprocess_order_placeholder(&mut chapter_dir_fmt, &vars)
            .context("预处理`order`占位符失败")?;

        let dir_fmt_parts: Vec<&str> = chapter_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).context("格式化目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }
        // 将格式化后的目录名拼接成完整的目录路径
        let mut chapter_download_dir = comic_download_dir.to_path_buf();
        for dir_name in dir_names {
            chapter_download_dir = chapter_download_dir.join(dir_name);
        }

        Ok(chapter_download_dir)
    }

    /// 预处理`fmt`中的`order`占位符
    ///
    /// ### 功能描述
    /// 标准的格式化(如`{order:0>4}`)会将宽度补齐应用于整个数字字符串
    /// 当`order`为`5.1`时，标准输出为`05.1`(总长度4)
    ///
    /// 本函数旨在实现**仅对整数部分补齐，小数部分追加在后**的效果
    /// 当`order`为`5.1`时，本函数会将其转换为`0005.1`(整数补齐至4位，小数保留)
    ///
    /// ### 处理流程
    /// 1. **解析数值**：从`vars`中提取`order`，将其拆分为整数部分和小数部分
    /// 2. **正则扫描**：使用正则查找模板中的`{order}`或`{order:xxx}`占位符，同时兼容`{{` 和 `}}`转义
    /// 3. **自定义格式化**：
    ///    - 提取占位符中的格式参数(如`0>4`)
    ///    - 仅将该参数应用于整数部分
    ///    - 若存在非零小数部分，将其追加到格式化后的整数后面
    /// 4. **原地替换**：将计算出的最终字符串(如 `0005.1`)直接替换掉原模板中的占位符
    ///
    /// ### 示例
    /// - 输入 fmt: `"{order:0>3} {chapter_title}"`, order: `"1.5"`
    /// - 处理后 fmt: `"001.5 {chapter_title}"`
    fn preprocess_order_placeholder(
        fmt: &mut String,
        vars: &HashMap<String, String>,
    ) -> anyhow::Result<()> {
        use strfmt::strfmt;

        let Some(order_str) = vars.get("order") else {
            return Ok(());
        };

        // 分离整数和小数
        let (int_part, frac_part) = match order_str.split_once('.') {
            Some((i, f)) => (i, f),
            None => (order_str.as_str(), ""),
        };
        let should_append_frac = !frac_part.is_empty() && frac_part != "0";

        // group 1: "{{" (转义左括号)
        // group 2: "}}" (转义右括号)
        // group 3: "{order...}" (真正的目标)
        // group 4: 冒号后的格式参数 (仅当 group 3 匹配时有效)
        let re = Regex::new(r"(\{\{)|(\}\})|(\{order(?::(.*?))?\})")?;

        // 执行替换
        let new_fmt = re.replace_all(fmt, |caps: &Captures| {
            // 遇到 {{，原样返回，消耗掉字符避免后续匹配误伤
            if caps.get(1).is_some() {
                return "{{".to_string();
            }
            // 遇到 }}，同理
            if caps.get(2).is_some() {
                return "}}".to_string();
            }
            // 匹配到了 {order...}
            // 此时 Group 4 是格式参数 (例如 "0>4")
            let fmt_spec = caps.get(4).map_or("", |m| m.as_str());

            // 构造临时模板 "{v:xxx}" 来格式化整数部分
            let int_fmt = format!("{{v:{fmt_spec}}}");
            let mut temp_vars = HashMap::new();
            temp_vars.insert("v".to_string(), int_part.to_string());

            let formatted_int = strfmt(&int_fmt, &temp_vars).unwrap_or(int_part.to_string());

            if should_append_frac {
                format!("{formatted_int}.{frac_part}")
            } else {
                formatted_int
            }
        });

        *fmt = new_fmt.to_string();

        Ok(())
    }
}
