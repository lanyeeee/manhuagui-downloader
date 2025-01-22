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

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum ExportCbzEvent {
    #[serde(rename_all = "camelCase")]
    Start {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    Progress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    End { uuid: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum ExportPdfEvent {
    #[serde(rename_all = "camelCase")]
    CreateStart {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    CreateProgress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    CreateEnd { uuid: String },

    #[serde(rename_all = "camelCase")]
    MergeStart {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    MergeProgress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    MergeEnd { uuid: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum UpdateDownloadedComicsEvent {
    #[serde(rename_all = "camelCase")]
    GettingComics { total: i64 },

    #[serde(rename_all = "camelCase")]
    ComicGot { current: i64, total: i64 },

    #[serde(rename_all = "camelCase")]
    DownloadTaskCreated,
}
