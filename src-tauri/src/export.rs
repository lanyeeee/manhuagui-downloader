mod cbz;
mod pdf;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
pub use cbz::cbz;
pub use pdf::pdf;

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

fn get_image_paths(images_dir: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
    let mut image_paths: Vec<PathBuf> = std::fs::read_dir(images_dir)
        .context(format!("读取目录`{}`失败", images_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_img())
        .collect();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(image_paths)
}
