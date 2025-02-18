use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::{
    download_manager::DownloadTaskState,
    types::{ChapterInfo, LogLevel},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Speed { speed: String },

    #[serde(rename_all = "camelCase")]
    Sleeping { chapter_id: i64, remaining_sec: u64 },
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

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub timestamp: String,
    pub level: LogLevel,
    pub fields: HashMap<String, serde_json::Value>,
    pub target: String,
    pub filename: String,
    #[serde(rename = "line_number")]
    pub line_number: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskEvent {
    pub state: DownloadTaskState,
    pub chapter_info: ChapterInfo,
    pub downloaded_img_count: u32,
    pub total_img_count: u32,
}
