use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use eyre::{OptionExt, WrapErr};
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::{
    sync::{watch, SemaphorePermit},
    task::JoinSet,
    time::sleep,
};

use crate::{
    downloader::{download_img_task::DownloadImgTask, download_task_state::DownloadTaskState},
    events::DownloadEvent,
    extensions::{AppHandleExt, ReportToStringChain},
    manhuagui_client::ManhuaguiClient,
    types::{ChapterInfo, Comic},
};

pub struct DownloadTask {
    pub app: AppHandle,
    pub comic: Arc<Comic>,
    pub chapter_info: Arc<ChapterInfo>,
    pub state_sender: watch::Sender<DownloadTaskState>,
    pub delete_sender: watch::Sender<()>,
    pub downloaded_img_count: Arc<AtomicU32>,
    pub total_img_count: Arc<AtomicU32>,
}

impl DownloadTask {
    pub fn new(app: AppHandle, mut comic: Comic, chapter_id: i64) -> eyre::Result<Arc<Self>> {
        comic
            .ensure_download_dir_fields(&app)
            .wrap_err(format!("漫画`{}`更新`download_dir`字段失败", comic.title))?;

        let chapter_info = comic
            .groups
            .iter()
            .flat_map(|(_, chapter_infos)| chapter_infos.iter())
            .find(|chapter_info| chapter_info.chapter_id == chapter_id)
            .cloned()
            .ok_or_eyre(format!("未找到章节ID为`{chapter_id}`的章节信息"))?;

        let (state_sender, _) = watch::channel(DownloadTaskState::Pending);
        let (delete_sender, _) = watch::channel(());

        let task = Arc::new(Self {
            app,
            comic: Arc::new(comic),
            chapter_info: Arc::new(chapter_info),
            state_sender,
            delete_sender,
            downloaded_img_count: Arc::new(AtomicU32::new(0)),
            total_img_count: Arc::new(AtomicU32::new(0)),
        });

        tauri::async_runtime::spawn(task.clone().process());

        Ok(task)
    }

    async fn process(self: Arc<Self>) {
        self.emit_download_task_create_event();

        let mut state_receiver = self.state_sender.subscribe();
        state_receiver.mark_changed();

        let mut delete_receiver = self.delete_sender.subscribe();

        let mut permit = None;
        let mut download_task_option = None;

        loop {
            let state = *state_receiver.borrow();
            let state_is_downloading = state == DownloadTaskState::Downloading;
            let state_is_pending = state == DownloadTaskState::Pending;

            let download_task = async {
                download_task_option
                    .get_or_insert_with(|| Box::pin(self.download_chapter()))
                    .await;
            };

            tokio::select! {
                () = download_task, if state_is_downloading && permit.is_some() => {
                    download_task_option = None;
                    if let Some(permit) = permit.take() {
                        drop(permit);
                    }
                }

                () = self.acquire_chapter_permit(&mut permit), if state_is_pending => {}

                _ = state_receiver.changed() => {
                    self.handle_state_change(&mut permit, &mut state_receiver).await;
                }

                _ = delete_receiver.changed() => {
                    self.handle_delete_receiver_change(&mut permit).await;
                    return;
                }
            }
        }
    }

    async fn download_chapter(self: &Arc<Self>) {
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
            let download_img_task =
                DownloadImgTask::new(self.clone(), url, i, temp_download_dir.clone());
            join_set.spawn(download_img_task.process());
        }
        join_set.join_all().await;

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "所有图片下载任务完成"
        );

        let downloaded_img_count = self.downloaded_img_count.load(Ordering::Relaxed);
        let total_img_count = self.total_img_count.load(Ordering::Relaxed);
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

        tracing::info!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "章节下载成功"
        );

        self.sleep_between_chapter().await;

        self.set_state(DownloadTaskState::Completed);
        self.emit_download_task_update_event();
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

        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(eyre::Report::from) {
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

    fn rename_temp_download_dir(&self, temp_download_dir: &Path) -> eyre::Result<()> {
        let chapter_download_dir = self
            .chapter_info
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        if chapter_download_dir.exists() {
            std::fs::remove_dir_all(chapter_download_dir)
                .wrap_err(format!("删除`{}`失败", chapter_download_dir.display()))?;
        }

        std::fs::rename(temp_download_dir, chapter_download_dir).wrap_err(format!(
            "将`{}`重命名为`{}`失败",
            temp_download_dir.display(),
            chapter_download_dir.display()
        ))?;

        Ok(())
    }

    async fn acquire_chapter_permit<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
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
            Some(permit) => Some(permit),
            None => match self
                .app
                .get_download_manager()
                .inner()
                .chapter_sem
                .acquire()
                .await
                .map_err(eyre::Report::from)
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
                    return;
                }
            },
        };

        if *self.state_sender.borrow() != DownloadTaskState::Pending {
            return;
        }

        if let Err(err) = self
            .state_sender
            .send(DownloadTaskState::Downloading)
            .map_err(eyre::Report::from)
        {
            let err_title = format!(
                "`{comic_title} - {group_name} - {chapter_title}`发送状态`Downloading`失败"
            );
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
        }
    }

    async fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        self.emit_download_task_update_event();
        let state = *state_receiver.borrow();

        if state == DownloadTaskState::Paused {
            sleep(Duration::from_millis(100)).await;
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
        } else if state == DownloadTaskState::Failed {
            sleep(Duration::from_millis(100)).await;
            if let Some(permit) = permit.take() {
                drop(permit);
            }
        }
    }

    async fn handle_delete_receiver_change<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        let chapter_id = self.chapter_info.chapter_id;
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;

        let _ = DownloadEvent::TaskDelete { chapter_id }.emit(&self.app);

        if permit.is_some() {
            sleep(Duration::from_millis(100)).await;
        }

        tracing::debug!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "章节下载任务已删除"
        );
    }

    async fn sleep_between_chapter(&self) {
        let chapter_id = self.chapter_info.chapter_id;
        let mut remaining_sec = self.app.get_config().read().chapter_download_interval_sec;
        while remaining_sec > 0 {
            let _ = DownloadEvent::Sleeping {
                chapter_id,
                remaining_sec,
            }
            .emit(&self.app);
            sleep(Duration::from_secs(1)).await;
            remaining_sec -= 1;
        }
    }

    pub fn set_state(&self, state: DownloadTaskState) {
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let chapter_title = &self.chapter_info.chapter_title;
        if let Err(err) = self.state_sender.send(state).map_err(eyre::Report::from) {
            let err_title =
                format!("`{comic_title} - {group_name} - {chapter_title}`发送状态`{state:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }
    }

    pub fn emit_download_task_update_event(&self) {
        let _ = DownloadEvent::TaskUpdate {
            chapter_id: self.chapter_info.chapter_id,
            state: *self.state_sender.borrow(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn emit_download_task_create_event(&self) {
        let _ = DownloadEvent::TaskCreate {
            state: *self.state_sender.borrow(),
            comic: Box::new(self.comic.as_ref().clone()),
            chapter_info: Box::new(self.chapter_info.as_ref().clone()),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.get_manhuagui_client().inner().clone()
    }
}
