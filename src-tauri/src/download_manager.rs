use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::time::sleep;
use tokio::{
    sync::{mpsc, Semaphore},
    task::JoinSet,
};

use crate::{
    config::Config, events::DownloadEvent, extensions::AnyhowErrorToStringChain,
    manhuagui_client::ManhuaguiClient, types::ChapterInfo,
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
    sender: Arc<mpsc::Sender<ChapterInfo>>,
    chapter_sem: Arc<Semaphore>,
    img_sem: Arc<Semaphore>,
    byte_per_sec: Arc<AtomicU64>,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel::<ChapterInfo>(32);

        let manager = DownloadManager {
            app: app.clone(),
            sender: Arc::new(sender),
            chapter_sem: Arc::new(Semaphore::new(1)),
            img_sem: Arc::new(Semaphore::new(10)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
        };

        tauri::async_runtime::spawn(Self::log_download_speed(app.clone()));
        tauri::async_runtime::spawn(Self::receiver_loop(app.clone(), receiver));

        manager
    }

    pub async fn submit_chapter(&self, chapter_info: ChapterInfo) -> anyhow::Result<()> {
        self.sender.send(chapter_info).await?;
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

    async fn receiver_loop(app: AppHandle, mut receiver: mpsc::Receiver<ChapterInfo>) {
        while let Some(chapter_info) = receiver.recv().await {
            let manager = app.state::<DownloadManager>().inner().clone();
            tauri::async_runtime::spawn(manager.process_chapter(chapter_info));
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)] // TODO: 重构此方法，减少代码行数
    async fn process_chapter(self, chapter_info: ChapterInfo) {
        let chapter_id = chapter_info.chapter_id;
        let comic_title = &chapter_info.comic_title;
        let group_name = &chapter_info.group_name;
        let chapter_title = &chapter_info.chapter_title;
        let err_prefix = format!("`{comic_title} - {group_name} - {chapter_title}`");
        // 发送章节排队事件
        let _ = DownloadEvent::ChapterPending {
            chapter_id,
            comic_title: comic_title.clone(),
            chapter_title: chapter_title.clone(),
        }
        .emit(&self.app);
        // 限制同时下载的章节数量
        let permit = match self
            .chapter_sem
            .acquire()
            .await
            .map_err(anyhow::Error::from)
        {
            Ok(permit) => permit,
            Err(err) => {
                let err_title = format!("{err_prefix}获取下载章节的semaphore失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                // 发送章节下载结束事件
                let _ = DownloadEvent::ChapterEnd { chapter_id }.emit(&self.app);
                return;
            }
        };
        // 获取此章节每张图片的下载链接
        let urls = match self.manhuagui_client().get_image_urls(&chapter_info).await {
            Ok(urls) => urls,
            Err(err) => {
                let err_title = format!("{err_prefix}获取图片链接失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                // 发送下载结束事件
                let _ = DownloadEvent::ChapterEnd { chapter_id }.emit(&self.app);
                return;
            }
        };
        // 总共需要下载的图片数量
        let total = urls.len() as u32;
        // 记录成功下载的图片数量
        let downloaded_count = Arc::new(AtomicU32::new(0));
        let mut join_set = JoinSet::new();
        // 创建临时下载目录
        let temp_download_dir = get_temp_download_dir(&self.app, &chapter_info);
        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(anyhow::Error::from) {
            let err_title = format!("{err_prefix}创建目录`{temp_download_dir:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            // 发送下载结束事件
            let _ = DownloadEvent::ChapterEnd { chapter_id }.emit(&self.app);
            return;
        }
        // 发送下载开始事件
        let _ = DownloadEvent::ChapterStart { chapter_id, total }.emit(&self.app);
        // 逐一创建下载任务
        for (i, url) in urls.into_iter().enumerate() {
            let manager = self.clone();
            let save_path = temp_download_dir.join(format!("{:03}.jpg", i + 1));
            let url = url.clone();
            let downloaded_count = downloaded_count.clone();
            // 创建下载任务
            join_set.spawn(manager.download_image(url, save_path, chapter_id, downloaded_count));
        }
        // 等待所有下载任务完成
        join_set.join_all().await;
        // 每个章节下载完成后，等待一段时间再下载下一个章节
        self.sleep_between_chapters(chapter_id).await;
        drop(permit);
        // 检查此章节的图片是否全部下载成功
        let downloaded_count = downloaded_count.load(Ordering::Relaxed);
        // 此章节的图片未全部下载成功
        if downloaded_count != total {
            let err_title = format!("{err_prefix}下载不完整");
            let err_msg = format!("总共有`{total}`张图片，但只下载了`{downloaded_count}`张");
            tracing::error!(err_title, message = err_msg);
            // 发送章节下载结束事件
            let _ = DownloadEvent::ChapterEnd { chapter_id }.emit(&self.app);
            return;
        }
        if let Err(err) = rename_temp_download_dir(&chapter_info, &temp_download_dir) {
            let err_title = format!("{err_prefix}重命名临时目录失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            // 发送章节下载结束事件
            let _ = DownloadEvent::ChapterEnd { chapter_id }.emit(&self.app);
            return;
        }
        // 章节下载成功
        tracing::info!(
            chapter_id,
            comic_title,
            group_name,
            chapter_title,
            "章节下载成功"
        );
        // 发送下载结束事件
        let _ = DownloadEvent::ChapterEnd { chapter_id }.emit(&self.app);
    }

    async fn sleep_between_chapters(&self, chapter_id: i64) {
        let mut remaining_sec = self
            .app
            .state::<RwLock<Config>>()
            .read()
            .download_interval_sec;
        while remaining_sec > 0 {
            // 发送章节休眠事件
            let _ = DownloadEvent::ChapterSleeping {
                chapter_id,
                remaining_sec,
            }
            .emit(&self.app);
            sleep(Duration::from_secs(1)).await;
            remaining_sec -= 1;
        }
    }

    async fn download_image(
        self,
        url: String,
        save_path: PathBuf,
        chapter_id: i64,
        current: Arc<AtomicU32>,
    ) {
        // 下载图片
        let permit = match self.img_sem.acquire().await.map_err(anyhow::Error::from) {
            Ok(permit) => permit,
            Err(err) => {
                let err_title = "获取下载图片的semaphore失败";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return;
            }
        };
        let image_data = match self.manhuagui_client().get_image_bytes(&url).await {
            Ok(data) => data,
            Err(err) => {
                let err_title = format!("下载图片`{url}`失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return;
            }
        };
        drop(permit);
        // 保存图片
        if let Err(err) = std::fs::write(&save_path, &image_data).map_err(anyhow::Error::from) {
            let err_title = format!("保存图片`{save_path:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }
        // 记录下载字节数
        self.byte_per_sec
            .fetch_add(image_data.len() as u64, Ordering::Relaxed);
        // 更新章节下载进度
        let current = current.fetch_add(1, Ordering::Relaxed) + 1;
        // 发送下载图片成功事件
        let _ = DownloadEvent::ImageSuccess {
            chapter_id,
            url,
            current,
        }
        .emit(&self.app);
    }

    fn manhuagui_client(&self) -> ManhuaguiClient {
        self.app.state::<ManhuaguiClient>().inner().clone()
    }
}

fn get_temp_download_dir(app: &AppHandle, chapter_info: &ChapterInfo) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .download_dir
        .join(&chapter_info.comic_title)
        .join(&chapter_info.group_name)
        .join(format!(".下载中-{}", chapter_info.prefixed_chapter_title)) // 以 `.下载中-` 开头，表示是临时目录
}

fn rename_temp_download_dir(
    chapter_info: &ChapterInfo,
    temp_download_dir: &Path,
) -> anyhow::Result<()> {
    let Some(parent) = temp_download_dir.parent() else {
        return Err(anyhow!("无法获取`{temp_download_dir:?}`的父目录"));
    };

    let download_dir = parent.join(&chapter_info.prefixed_chapter_title);

    if download_dir.exists() {
        std::fs::remove_dir_all(&download_dir)
            .context(format!("删除目录`{download_dir:?}`失败"))?;
    }

    std::fs::rename(temp_download_dir, &download_dir).context(format!(
        "将`{temp_download_dir:?}`重命名为`{download_dir:?}`失败"
    ))?;

    Ok(())
}
