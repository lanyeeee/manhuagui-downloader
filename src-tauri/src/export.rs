use std::{
    io::Write,
    path::PathBuf,
    sync::{atomic::AtomicU32, Arc},
};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::Config,
    events::ExportCbzEvent,
    types::{ChapterInfo, Comic, ComicInfo},
};

enum Archive {
    Cbz,
}
impl Archive {
    pub fn extension(&self) -> &str {
        match self {
            Archive::Cbz => "cbz",
        }
    }
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
pub fn cbz(app: &AppHandle, comic: Comic) -> anyhow::Result<()> {
    // 获取已下载的章节
    let downloaded_chapters = comic
        .groups
        .into_iter()
        .flat_map(|(_, chapters)| chapters)
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .collect::<Vec<_>>();
    // 生成格式化的xml
    let cfg = yaserde::ser::Config {
        perform_indent: true,
        ..Default::default()
    };
    let event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始导出cbz事件
    let _ = ExportCbzEvent::Start {
        uuid: event_uuid.clone(),
        comic_title: comic.title,
        total: downloaded_chapters.len() as u32,
    }
    .emit(app);
    // 用来记录导出进度
    let current = Arc::new(AtomicU32::new(0));
    // 并发处理
    let downloaded_chapters = downloaded_chapters.into_par_iter();
    downloaded_chapters.try_for_each(|chapter_info| -> anyhow::Result<()> {
        let chapter_title = chapter_info.chapter_title.clone();
        let prefixed_chapter_title = chapter_info.prefixed_chapter_title.clone();
        let group_name = chapter_info.group_name.clone();
        let chapter_download_dir = get_chapter_download_dir(app, &chapter_info);
        let chapter_export_dir = get_chapter_export_dir(app, &chapter_info, &Archive::Cbz);
        let comic_info_path = chapter_export_dir.join("ComicInfo.xml");
        let err_prefix = format!("`{group_name} - {chapter_title}`");
        // 生成ComicInfo
        let comic_info = ComicInfo::from(
            chapter_info,
            &comic.authors,
            &comic.genres,
            comic.intro.clone(),
        );
        // 序列化ComicInfo为xml
        let comic_info_xml = yaserde::ser::to_string_with_config(&comic_info, &cfg)
            .map_err(|err_msg| anyhow!("{err_prefix}序列化`{comic_info_path:?}`失败: {err_msg}"))?;
        // 保证导出目录存在
        std::fs::create_dir_all(&chapter_export_dir)
            .context(format!("{err_prefix}创建目录`{chapter_export_dir:?}`失败"))?;
        // 创建cbz文件
        let extension = Archive::Cbz.extension();
        let zip_path = chapter_export_dir.join(format!("{prefixed_chapter_title}.{extension}"));
        let zip_file = std::fs::File::create(&zip_path)
            .context(format!("{err_prefix}创建文件`{zip_path:?}`失败"))?;
        let mut zip_writer = ZipWriter::new(zip_file);
        // 把ComicInfo.xml写入cbz
        zip_writer
            .start_file("ComicInfo.xml", SimpleFileOptions::default())
            .context(format!(
                "{err_prefix}在`{zip_path:?}`创建`ComicInfo.xml`失败"
            ))?;
        zip_writer
            .write_all(comic_info_xml.as_bytes())
            .context("{err_prefix}写入`ComicInfo.xml`失败")?;
        // 遍历下载目录，将文件写入cbz
        let entries = std::fs::read_dir(&chapter_download_dir)
            .context(format!(
                "{err_prefix}读取目录`{chapter_download_dir:?}`失败"
            ))?
            .filter_map(Result::ok);
        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => continue,
            };
            // 将文件写入cbz
            zip_writer
                .start_file(&filename, SimpleFileOptions::default())
                .context(format!(
                    "{err_prefix}在`{zip_path:?}`创建`{filename:?}`失败"
                ))?;
            let mut file = std::fs::File::open(&path).context(format!("打开 {path:?} 失败"))?;
            std::io::copy(&mut file, &mut zip_writer)
                .context(format!("{err_prefix}将`{path:?}`写入`{zip_path:?}`失败"))?;
        }

        zip_writer
            .finish()
            .context(format!("{err_prefix}关闭`{zip_path:?}`失败"))?;
        // 更新导出cbz的进度
        let current = current.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // 发送导出cbz进度事件
        let _ = ExportCbzEvent::Progress {
            uuid: event_uuid.clone(),
            current,
        }
        .emit(app);
        Ok(())
    })?;
    // 发送导出cbz完成事件
    let _ = ExportCbzEvent::End { uuid: event_uuid }.emit(app);

    Ok(())
}
fn get_chapter_export_dir(
    app: &AppHandle,
    chapter_info: &ChapterInfo,
    archive: &Archive,
) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .export_dir
        .join(&chapter_info.comic_title)
        .join(archive.extension())
        .join(&chapter_info.group_name)
}

fn get_chapter_download_dir(app: &AppHandle, chapter_info: &ChapterInfo) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .download_dir
        .join(&chapter_info.comic_title)
        .join(&chapter_info.group_name)
        .join(&chapter_info.prefixed_chapter_title)
}
