mod cbz;
mod pdf;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use cbz::cbz;
use eyre::WrapErr;
pub use pdf::pdf;
use tracing::instrument;

use crate::{extensions::PathIsImg, types::ChapterInfo};

enum Archive {
    Cbz,
    Pdf,
}

impl Archive {
    fn extension(&self) -> &str {
        match self {
            Archive::Cbz => "cbz",
            Archive::Pdf => "pdf",
        }
    }
}

/// 获取已下载的章节
fn get_downloaded_chapters(groups: HashMap<String, Vec<ChapterInfo>>) -> Vec<ChapterInfo> {
    groups
        .into_iter()
        .flat_map(|(_, chapters)| chapters)
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .collect::<Vec<_>>()
}

#[instrument(level = "error", skip_all, fields(images_dir = %images_dir.display()))]
fn get_image_paths(images_dir: &Path) -> eyre::Result<Vec<PathBuf>> {
    let mut image_paths: Vec<PathBuf> = std::fs::read_dir(images_dir)
        .wrap_err(format!("读取目录`{}`失败", images_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_img())
        .collect();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(image_paths)
}
