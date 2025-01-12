use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    ChapterPending {
        chapter_id: i64,
        comic_title: String,
        chapter_title: String,
    },

    #[serde(rename_all = "camelCase")]
    ChapterControlRisk { chapter_id: i64, retry_after: u32 },

    #[serde(rename_all = "camelCase")]
    ChapterStart { chapter_id: i64, total: u32 },

    #[serde(rename_all = "camelCase")]
    ChapterEnd {
        chapter_id: i64,
        err_msg: Option<String>,
    },

    #[serde(rename_all = "camelCase")]
    ImageSuccess {
        chapter_id: i64,
        url: String,
        current: u32,
    },

    #[serde(rename_all = "camelCase")]
    ImageError {
        chapter_id: i64,
        url: String,
        err_msg: String,
    },

    #[serde(rename_all = "camelCase")]
    Speed { speed: String },
}
