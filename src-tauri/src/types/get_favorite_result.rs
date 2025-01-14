use anyhow::Context;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::extensions::ToAnyhow;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GetFavoriteResult {
    comics: Vec<ComicInFavorite>,
    current: i64,
    total: i64,
}

impl GetFavoriteResult {
    pub fn from_html(html: &str) -> anyhow::Result<GetFavoriteResult> {
        let document = Html::parse_document(html);
        let mut comics = Vec::new();
        for book_div in document.select(&Selector::parse(".dy_content_li").to_anyhow()?) {
            let comic = ComicInFavorite::from_div(&book_div)?;
            comics.push(comic);
        }

        let current = match document
            .select(&Selector::parse(".current").to_anyhow()?)
            .next()
        {
            Some(span) => span
                .text()
                .next()
                .context("没有在当前页码的<span>中找到文本")?
                .parse::<i64>()
                .context("当前页码不是整数")?,
            None => 1,
        };

        // 如果没有找到总页数的span，说明只有一页
        let total = match document
            .select(&Selector::parse(".flickr.right > span").to_anyhow()?)
            .next()
        {
            Some(span) => span
                .text()
                .next()
                .context("没有在总页数的<span>中找到文本")?
                .trim_start_matches("共")
                .trim_end_matches("记录")
                .parse::<i64>()
                .context("总页数不是整数")?,
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
    id: i64,
    /// 漫画标题
    title: String,
    /// 漫画封面链接
    cover: String,
    /// 最近更新时间，两种格式
    /// - 2024-12-13
    /// - x分钟前
    last_update: String,
    /// 上次阅读时间，两种格式
    /// - 2024-12-13
    /// - x分钟前
    last_read: String,
}

impl ComicInFavorite {
    pub fn from_div(div: &ElementRef) -> anyhow::Result<ComicInFavorite> {
        let a = div
            .select(&Selector::parse(".dy_content_li h3 > a").to_anyhow()?)
            .next()
            .context("没有找到标题相关的<a>")?;

        let id = a
            .value()
            .attr("href")
            .context("没有在标题和链接的<a>中找到href属性")?
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .context("漫画id不是整数")?;

        let title = a
            .text()
            .next()
            .context("没有在标题和链接的<a>中找到文本")?
            .trim()
            .to_string();

        let cover_src = div
            .select(&Selector::parse(".dy_img img").to_anyhow()?)
            .next()
            .context("没有找到封面的<img>")?
            .value()
            .attr("src")
            .context("没有在封面的<img>中找到src属性")?;
        let cover = format!("https:{cover_src}");

        let last_update = div
            .select(&Selector::parse(".dy_r > p > em:nth-child(2)").to_anyhow()?)
            .next()
            .context("没有找到最近更新时间<em>")?
            .text()
            .next()
            .context("没有在最近更新时间<em>中找到文本")?
            .trim()
            .to_string();

        let last_read = div
            .select(&Selector::parse(".dy_r > p > em:nth-child(2)").to_anyhow()?)
            .nth(1)
            .context("没有找到上次阅读时间<em>")?
            .text()
            .next()
            .context("没有在上次阅读时间<em>中找到文本")?
            .trim()
            .to_string();

        Ok(ComicInFavorite {
            id,
            title,
            cover,
            last_update,
            last_read,
        })
    }
}
