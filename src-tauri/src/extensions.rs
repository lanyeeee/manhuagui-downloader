use anyhow::anyhow;
use parking_lot::RwLock;
use reqwest::Response;
use reqwest_middleware::RequestBuilder;
use scraper::error::SelectorErrorKind;
use tauri::{Manager, State};

use crate::{
    config::Config, downloader::download_manager::DownloadManager,
    manhuagui_client::ManhuaguiClient,
};

pub trait AnyhowErrorToStringChain {
    /// 将 `anyhow::Error` 转换为chain格式
    /// # Example
    /// 0: error message
    /// 1: error message
    /// 2: error message
    fn to_string_chain(&self) -> String;
}

impl AnyhowErrorToStringChain for anyhow::Error {
    fn to_string_chain(&self) -> String {
        use std::fmt::Write;
        self.chain()
            .enumerate()
            .fold(String::new(), |mut output, (i, e)| {
                let _ = writeln!(output, "{i}: {e}");
                output
            })
    }
}

pub trait ToAnyhow<T> {
    fn to_anyhow(self) -> anyhow::Result<T>;
}

impl<T> ToAnyhow<T> for Result<T, SelectorErrorKind<'_>> {
    fn to_anyhow(self) -> anyhow::Result<T> {
        self.map_err(|e| anyhow!(e.to_string()))
    }
}

pub trait SendWithTimeoutMsg {
    /// 发送请求并处理超时错误
    ///
    /// - 如果遇到超时错误，返回带有用户友好信息的错误
    /// - 否则返回原始错误
    async fn send_with_timeout_msg(self) -> anyhow::Result<Response>;
}

impl SendWithTimeoutMsg for RequestBuilder {
    async fn send_with_timeout_msg(self) -> anyhow::Result<Response> {
        self.send().await.map_err(|e| {
            if e.is_timeout() || e.is_middleware() {
                anyhow::Error::from(e).context(
                    "网络连接超时，可能是未使用代理或IP被封，请使用代理或切换代理线路后重试",
                )
            } else {
                anyhow::Error::from(e)
            }
        })
    }
}

pub trait AppHandleExt {
    fn get_config(&self) -> State<'_, RwLock<Config>>;
    fn get_manhuagui_client(&self) -> State<'_, ManhuaguiClient>;
    fn get_download_manager(&self) -> State<'_, DownloadManager>;
}

impl AppHandleExt for tauri::AppHandle {
    fn get_config(&self) -> State<'_, RwLock<Config>> {
        self.state::<RwLock<Config>>()
    }
    fn get_manhuagui_client(&self) -> State<'_, ManhuaguiClient> {
        self.state::<ManhuaguiClient>()
    }
    fn get_download_manager(&self) -> State<'_, DownloadManager> {
        self.state::<DownloadManager>()
    }
}

pub trait WalkDirEntryExt {
    fn is_comic_metadata(&self) -> bool;
    fn is_chapter_metadata(&self) -> bool;
}
impl WalkDirEntryExt for walkdir::DirEntry {
    fn is_comic_metadata(&self) -> bool {
        if !self.file_type().is_file() {
            return false;
        }
        if self.file_name() != "元数据.json" {
            return false;
        }

        true
    }

    fn is_chapter_metadata(&self) -> bool {
        if !self.file_type().is_file() {
            return false;
        }
        if self.file_name() != "章节元数据.json" {
            return false;
        }

        true
    }
}

pub trait PathIsImg {
    /// 判断路径是否为图片文件
    fn is_img(&self) -> bool;
}

impl PathIsImg for std::path::Path {
    fn is_img(&self) -> bool {
        self.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase)
            .is_some_and(|ext| matches!(ext.as_str(), "jpg"))
    }
}
