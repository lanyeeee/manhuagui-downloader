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
use tracing::instrument;

use crate::{
    downloader::{download_task::DownloadTask, download_task_state::DownloadTaskState},
    extensions::{AppHandleExt, EyreReportToMessage},
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

    #[instrument(
        level = "error",
        skip_all,
        fields(
            index = self.index,
            url = self.url,
            comic_id = self.download_task.chapter_info.comic_id,
            comic_title = self.download_task.chapter_info.comic_title,
            group_name = self.download_task.chapter_info.group_name,
            chapter_id = self.download_task.chapter_info.chapter_id,
            order = self.download_task.chapter_info.order
        )
    )]
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

    #[instrument(level = "error", skip_all)]
    async fn download_img(&self) {
        let url = &self.url;

        let save_path = self
            .temp_download_dir
            .join(format!("{:03}.jpg", self.index + 1));
        if save_path.exists() {
            self.download_task
                .downloaded_img_count
                .fetch_add(1, Ordering::Relaxed);

            self.download_task.emit_download_task_update_event();

            tracing::trace!("图片已存在，跳过下载");
            return;
        }

        tracing::trace!("开始下载图片");

        let img_data = match self.manhuagui_client().get_img_bytes(url).await {
            Ok(data) => data,
            Err(err) => {
                let err_title = "下载图片失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                return;
            }
        };

        tracing::trace!("图片成功下载到内存");

        if let Err(err) = std::fs::write(&save_path, &img_data).map_err(eyre::Report::from) {
            let err_title = "保存图片失败";
            let message = err.to_message();
            tracing::error!(err_title, message);
            return;
        }

        tracing::trace!("图片成功保存到磁盘");

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

    #[instrument(level = "error", skip_all)]
    async fn acquire_img_permit<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        tracing::trace!("图片开始排队");

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
                    let err_title = "获取下载图片的permit失败";
                    let message = err.to_message();
                    tracing::error!(err_title, message);
                    return;
                }
            },
        };
    }

    #[instrument(level = "error", skip_all)]
    async fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) {
        let state = *state_receiver.borrow();
        if state == DownloadTaskState::Paused {
            sleep(Duration::from_millis(100)).await;
            tracing::trace!("图片暂停下载");
            if let Some(permit) = permit.take() {
                drop(permit);
            }
        } else if state == DownloadTaskState::Failed {
            sleep(Duration::from_millis(100)).await;
            tracing::trace!("图片下载失败");
            if let Some(permit) = permit.take() {
                drop(permit);
            }
        }
    }

    #[instrument(level = "error", skip_all)]
    async fn handle_delete_receiver_change<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        if permit.is_some() {
            sleep(Duration::from_millis(100)).await;
        }

        tracing::trace!("图片下载任务已删除");
    }

    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.get_manhuagui_client().inner().clone()
    }
}
