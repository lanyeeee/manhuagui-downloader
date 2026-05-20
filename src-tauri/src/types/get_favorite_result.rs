use std::{collections::HashMap, path::PathBuf};

use eyre::{OptionExt, WrapErr};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::{extensions::ToEyre, utils};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GetFavoriteResult {
    comics: Vec<ComicInFavorite>,
    current: i64,
    total: i64,
}

impl GetFavoriteResult {
    pub fn from_html(app: &AppHandle, html: &str) -> eyre::Result<GetFavoriteResult> {
        let id_to_dir_map =
            utils::create_id_to_dir_map(app).wrap_err("创建漫画ID到下载目录映射失败")?;

        let document = Html::parse_document(html);
        let mut comics = Vec::new();
        for book_div in document.select(&Selector::parse(".dy_content_li").to_eyre()?) {
            let comic = ComicInFavorite::from_div(&book_div, &id_to_dir_map)?;
            comics.push(comic);
        }

        let current = match document
            .select(&Selector::parse(".current").to_eyre()?)
            .next()
        {
            Some(span) => span
                .text()
                .next()
                .ok_or_eyre("没有在当前页码的<span>中找到文本")?
                .parse::<i64>()
                .wrap_err("当前页码不是整数")?,
            None => 1,
        };

        // 如果没有找到总页数的span，说明只有一页
        let total = match document
            .select(&Selector::parse(".flickr.right > span").to_eyre()?)
            .next()
        {
            Some(span) => span
                .text()
                .next()
                .ok_or_eyre("没有在总页数的<span>中找到文本")?
                .trim_start_matches("共")
                .trim_end_matches("记录")
                .parse::<i64>()
                .wrap_err("总页数不是整数")?,
            None => 1,
        };

        Ok(GetFavoriteResult {
            comics,
            current,
            total,
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInFavorite {
    /// 漫画id
    pub id: i64,
    /// 漫画标题
    pub title: String,
    /// 漫画封面链接
    pub cover: String,
    /// 最近更新时间，两种格式
    /// - 2024-12-13
    /// - x分钟前
    pub last_update: String,
    /// 上次阅读时间，两种格式
    /// - 2024-12-13
    /// - x分钟前
    pub last_read: String,
    /// 是否已下载
    pub is_downloaded: bool,
    /// 漫画的下载目录
    pub comic_download_dir: PathBuf,
}

impl ComicInFavorite {
    pub fn from_div(
        div: &ElementRef,
        id_to_dir_map: &HashMap<i64, PathBuf>,
    ) -> eyre::Result<ComicInFavorite> {
        let a = div
            .select(&Selector::parse(".dy_content_li h3 > a").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到标题相关的<a>")?;

        let id = a
            .value()
            .attr("href")
            .ok_or_eyre("没有在标题和链接的<a>中找到href属性")?
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .wrap_err("漫画id不是整数")?;

        let title = a
            .text()
            .next()
            .ok_or_eyre("没有在标题和链接的<a>中找到文本")?
            .trim()
            .to_string();

        let cover_src = div
            .select(&Selector::parse(".dy_img img").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到封面的<img>")?
            .value()
            .attr("src")
            .ok_or_eyre("没有在封面的<img>中找到src属性")?;
        let cover = format!("https:{cover_src}");

        let last_update = div
            .select(&Selector::parse(".dy_r > p > em:nth-child(2)").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到最近更新时间<em>")?
            .text()
            .next()
            .ok_or_eyre("没有在最近更新时间<em>中找到文本")?
            .trim()
            .to_string();

        let last_read = div
            .select(&Selector::parse(".dy_r > p > em:nth-child(2)").to_eyre()?)
            .nth(1)
            .ok_or_eyre("没有找到上次阅读时间<em>")?
            .text()
            .next()
            .ok_or_eyre("没有在上次阅读时间<em>中找到文本")?
            .trim()
            .to_string();

        let mut comic = ComicInFavorite {
            id,
            title,
            cover,
            last_update,
            last_read,
            is_downloaded: false,
            comic_download_dir: PathBuf::new(),
        };

        comic.update_fields(id_to_dir_map);

        Ok(comic)
    }

    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<i64, PathBuf>) {
        if let Some(comic_download_dir) = id_to_dir_map.get(&self.id) {
            self.comic_download_dir = comic_download_dir.clone();
            self.is_downloaded = true;
        }
    }
}
