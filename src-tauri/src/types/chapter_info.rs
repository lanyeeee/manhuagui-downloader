use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::extensions::AppHandleExt;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    /// 章节id
    pub chapter_id: i64,
    /// 章节标题
    pub chapter_title: String,
    /// 此章节有多少页
    pub chapter_size: i64,
    /// 以order为前缀的章节标题
    pub prefixed_chapter_title: String,
    /// 漫画id
    pub comic_id: i64,
    /// 漫画标题
    pub comic_title: String,
    /// 组名(单话、单行本、番外篇)
    pub group_name: String,
    /// 此章节对应的group有多少章节
    pub group_size: i64,
    /// 此章节在group中的顺序
    pub order: f64,
    /// 漫画状态(连载中/已完结)
    pub comic_status: String,
    /// 是否已下载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
}

impl ChapterInfo {
    pub fn get_is_downloaded(
        app: &AppHandle,
        comic_title: &str,
        group_name: &str,
        prefixed_chapter_title: &str,
    ) -> bool {
        app.get_config()
            .read()
            .download_dir
            .join(comic_title)
            .join(group_name)
            .join(prefixed_chapter_title)
            .exists()
    }
}
