use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::{extensions::AppHandleExt, types::Comic, utils};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    /// 章节id
    pub chapter_id: i64,
    /// 章节标题
    pub chapter_title: String,
    /// 此章节有多少页
    pub chapter_size: i64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_download_dir: Option<PathBuf>,
}

impl ChapterInfo {
    pub fn save_metadata(&self) -> anyhow::Result<()> {
        let mut chapter_info = self.clone();
        // 将is_downloaded和chapter_download_dir字段设置为None
        // 这样能使这些字段在序列化时被忽略
        chapter_info.is_downloaded = None;
        chapter_info.chapter_download_dir = None;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .context("`chapter_download_dir`字段为`None`")?;
        let metadata_path = chapter_download_dir.join("章节元数据.json");

        std::fs::create_dir_all(chapter_download_dir)
            .context(format!("创建目录`{}`失败", chapter_download_dir.display()))?;

        let chapter_json =
            serde_json::to_string_pretty(&chapter_info).context("将ChapterInfo序列化为json失败")?;

        std::fs::write(&metadata_path, chapter_json)
            .context(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    pub fn get_temp_download_dir(&self) -> anyhow::Result<PathBuf> {
        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .context("`chapter_download_dir`字段为`None`")?;

        let chapter_download_dir_name = chapter_download_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context(format!(
                "获取`{}`的目录名失败",
                chapter_download_dir.display()
            ))?;

        let parent = chapter_download_dir.parent().context(format!(
            "`{}`的父目录不存在",
            chapter_download_dir.display()
        ))?;

        let temp_download_dir = parent.join(format!(".下载中-{chapter_download_dir_name}"));
        Ok(temp_download_dir)
    }

    pub fn get_chapter_relative_dir(&self, comic: &Comic) -> anyhow::Result<PathBuf> {
        let comic_download_dir = comic
            .comic_download_dir
            .as_ref()
            .context("`comic_download_dir`字段为`None`")?;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .context("`chapter_download_dir`字段为`None`")?;

        let relative_dir = chapter_download_dir
            .strip_prefix(comic_download_dir)
            .context(format!(
                "无法从路径`{}`中移除前缀`{}`",
                chapter_download_dir.display(),
                comic_download_dir.display()
            ))?;

        Ok(relative_dir.to_path_buf())
    }

    pub fn get_chapter_download_dir_by_fmt(
        app: &AppHandle,
        comic_download_dir: &Path,
        fmt_params: &ChapterDirFmtParams,
    ) -> anyhow::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .context("将ChapterDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .context("ChapterDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();

        let mut chapter_dir_fmt = app.get_config().read().chapter_dir_fmt.clone();
        Self::preprocess_order_placeholder(&mut chapter_dir_fmt, &vars)
            .context("预处理`order`占位符失败")?;

        let dir_fmt_parts: Vec<&str> = chapter_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).context("格式化目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }

        let mut chapter_download_dir = comic_download_dir.to_path_buf();
        for dir_name in dir_names {
            chapter_download_dir = chapter_download_dir.join(dir_name);
        }

        Ok(chapter_download_dir)
    }

    pub fn preprocess_order_placeholder(
        fmt: &mut String,
        vars: &HashMap<String, String>,
    ) -> anyhow::Result<()> {
        use strfmt::strfmt;

        let Some(order_str) = vars.get("order") else {
            return Ok(());
        };

        let (int_part, frac_part) = match order_str.split_once('.') {
            Some((i, f)) => (i, f),
            None => (order_str.as_str(), ""),
        };
        let should_append_frac = !frac_part.is_empty() && frac_part != "0";

        let re = Regex::new(r"(\{\{)|(\}\})|(\{order(?::(.*?))?\})")?;

        let new_fmt = re.replace_all(fmt, |caps: &Captures| {
            if caps.get(1).is_some() {
                return "{{".to_string();
            }
            if caps.get(2).is_some() {
                return "}}".to_string();
            }

            let fmt_spec = caps.get(4).map_or("", |m| m.as_str());
            let int_fmt = format!("{{v:{fmt_spec}}}");
            let mut temp_vars = HashMap::new();
            temp_vars.insert("v".to_string(), int_part.to_string());

            let formatted_int = strfmt(&int_fmt, &temp_vars).unwrap_or(int_part.to_string());

            if should_append_frac {
                format!("{formatted_int}.{frac_part}")
            } else {
                formatted_int
            }
        });

        *fmt = new_fmt.to_string();

        Ok(())
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize, Type)]
pub struct ChapterDirFmtParams {
    pub comic_id: i64,
    pub comic_title: String,
    pub comic_subtitle: String,
    pub pub_year: i64,
    pub region: String,
    pub author: String,
    pub group_name: String,
    pub chapter_id: i64,
    pub chapter_title: String,
    pub order: f64,
}
