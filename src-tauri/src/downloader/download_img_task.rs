use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use tauri::AppHandle;
use tokio::{
    sync::{watch, SemaphorePermit},
    time::sleep,
};

use crate::{
    downloader::{download_task::DownloadTask, download_task_state::DownloadTaskState},
    extensions::{AppHandleExt, ReportToStringChain},
    manhuagui_client::ManhuaguiClient,
};

pub struct DownloadImgTask {
    app: AppHandle,
    download_task: Arc<DownloadTask>,
    url: String,
    index: usize,
    temp_download_dir: PathBuf,
}

impl DownloadImgTask {
    pub fn new(
        download_task: Arc<DownloadTask>,
        url: String,
        index: usize,
        temp_download_dir: PathBuf,
    ) -> Self {
        Self {
            app: download_task.app.clone(),
            download_task,
            url,
            index,
            temp_download_dir,
        }
    }

    pub async fn process(self) {
        let download_img_task = self.download_img();
        tokio::pin!(download_img_task);

        let mut state_receiver = self.download_task.state_sender.subscribe();
        state_receiver.mark_changed();

        let mut delete_receiver = self.download_task.delete_sender.subscribe();

        let mut permit = None;

        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            tokio::select! {
                () = &mut download_img_task, if state_is_downloading && permit.is_some() => break,

                () = self.acquire_img_permit(&mut permit), if state_is_downloading && permit.is_none() => {}

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

    async fn download_img(&self) {
        let url = &self.url;
        let chapter_id = self.download_task.chapter_info.chapter_id;
        let comic_title = &self.download_task.chapter_info.comic_title;
        let group_name = &self.download_task.chapter_info.group_name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        let save_path = self
            .temp_download_dir
            .join(format!("{:03}.jpg", self.index + 1));
        if save_path.exists() {
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

        if let Err(err) = std::fs::write(&save_path, &img_data).map_err(eyre::Report::from) {
            let err_title = format!("保存图片`{}`失败", save_path.display());
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }

        tracing::trace!(chapter_id, url, "图片成功保存到`{}`", save_path.display());

        self.app
            .get_download_manager()
            .byte_per_sec
            .fetch_add(img_data.len() as u64, Ordering::Relaxed);

        self.download_task
            .downloaded_img_count
            .fetch_add(1, Ordering::Relaxed);

        self.download_task.emit_download_task_update_event();

        let img_download_interval_sec = self.app.get_config().read().img_download_interval_sec;
        sleep(Duration::from_secs(img_download_interval_sec)).await;
    }

    async fn acquire_img_permit<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        let url = &self.url;
        let chapter_id = self.download_task.chapter_info.chapter_id;
        let comic_title = &self.download_task.chapter_info.comic_title;
        let group_name = &self.download_task.chapter_info.group_name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            url,
            "图片开始排队"
        );

        *permit = match permit.take() {
            Some(permit) => Some(permit),
            None => match self
                .app
                .get_download_manager()
                .inner()
                .img_sem
                .acquire()
                .await
                .map_err(eyre::Report::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title = format!(
                        "`{comic_title} - {group_name} - {chapter_title}`获取下载图片的permit失败"
                    );
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    return;
                }
            },
        };
    }

    async fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) {
        let url = &self.url;
        let chapter_id = self.download_task.chapter_info.chapter_id;
        let comic_title = &self.download_task.chapter_info.comic_title;
        let group_name = &self.download_task.chapter_info.group_name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        let state = *state_receiver.borrow();
        if state == DownloadTaskState::Paused {
            sleep(Duration::from_millis(100)).await;
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
        } else if state == DownloadTaskState::Failed {
            sleep(Duration::from_millis(100)).await;
            tracing::trace!(
                chapter_id,
                comic_title,
                group_name,
                chapter_title,
                url,
                "图片下载失败"
            );
            if let Some(permit) = permit.take() {
                drop(permit);
            }
        }
    }

    async fn handle_delete_receiver_change<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        let url = &self.url;
        let chapter_id = self.download_task.chapter_info.chapter_id;
        let comic_title = &self.download_task.chapter_info.comic_title;
        let group_name = &self.download_task.chapter_info.group_name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        if permit.is_some() {
            sleep(Duration::from_millis(100)).await;
        }

        tracing::trace!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            url,
            "图片下载任务已删除"
        );
    }

    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.get_manhuagui_client().inner().clone()
    }
}
