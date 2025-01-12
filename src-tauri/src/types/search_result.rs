use anyhow::Context;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::extensions::ToAnyhow;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    comics: Vec<ComicInSearch>,
    current: i64,
    total: i64,
}

impl SearchResult {
    pub fn from_html(html: &str) -> anyhow::Result<SearchResult> {
        let document = Html::parse_document(html);
        let book_result_selector = Selector::parse(".book-result .cf").to_anyhow()?;

        let mut comics = Vec::new();
        for book_li in document.select(&book_result_selector) {
            let comic = ComicInSearch::from_li(&book_li)?;
            comics.push(comic);
        }

        let current = match document
            .select(&Selector::parse(".current").to_anyhow()?)
            .next()
        {
            Some(span) => span
                .text()
                .next()
                .context("没有在当前页码的span中找到文本")?
                .parse::<i64>()
                .context("当前页码不是整数")?,
            None => 1,
        };

        let total = document
            .select(&Selector::parse(".result-count strong").to_anyhow()?)
            .nth(1)
            .context("没有找到总结果数的<strong>")?
            .text()
            .next()
            .context("没有在总结果数的<strong>中找到文本")?
            .parse::<i64>()
            .context("总结果数不是整数")?;

        Ok(SearchResult {
            comics,
            current,
            total,
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearch {
    /// 漫画id
    id: i64,
    /// 漫画标题
    title: String,
    /// 漫画副标题
    subtitle: Option<String>,
    /// 封面链接
    cover: String,
    /// 漫画状态(连载中/已完结)
    status: String,
    /// 上次更新时间
    update_time: String,
    /// 出版年份
    year: i64,
    /// 地区
    region: String,
    /// 类型
    genres: Vec<String>,
    /// 作者
    authors: Vec<String>,
    /// 漫画别名
    aliases: Vec<String>,
    /// 简介
    intro: String,
}

impl ComicInSearch {
    pub fn from_li(li: &ElementRef) -> anyhow::Result<ComicInSearch> {
        let book_detail_div = li
            .select(&Selector::parse(".book-detail").to_anyhow()?)
            .next()
            .context("没有找到书籍详情的<div>")?;

        let dt = book_detail_div
            .select(&Selector::parse("dt").to_anyhow()?)
            .next()
            .context("没有找到漫画标题和链接的<dt>")?;
        let (id, title, subtitle) = get_id_and_title_and_subtitle(dt)?;

        let cover_src = li
            .select(&Selector::parse(".book-cover img").to_anyhow()?)
            .next()
            .context("没有找到封面的<img>")?
            .value()
            .attr("src")
            .context("没有在封面的<img>中找到src属性")?;
        let cover = format!("https:{cover_src}");

        let dds = book_detail_div
            .select(&Selector::parse("dd").to_anyhow()?)
            .collect::<Vec<_>>();

        let status_dd = dds.first().context("没有找到漫画状态和更新时间的dd")?;
        let (status, update_time) = get_status_and_update_time(status_dd)?;

        let info_dd = dds.get(1).context("没有找到年份、地区、类型的dd")?;
        let (year, region, genres) = get_year_and_region_and_genres(info_dd)?;

        let authors = dds
            .get(2)
            .context("没有找到作者的<dd>")?
            .select(&Selector::parse("a").to_anyhow()?)
            .filter_map(|a| a.value().attr("title").map(str::to_string))
            .collect::<Vec<_>>();

        let aliases = dds
            .get(3)
            .context("没有找到别名的<dd>")?
            .select(&Selector::parse("a").to_anyhow()?)
            .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
            .collect::<Vec<_>>();

        let intro = book_detail_div
            .select(&Selector::parse(".intro span").to_anyhow()?)
            .next()
            .context("没有找到简介的<span>")?
            .text()
            .nth(1)
            .context("没有在简介的<span>中找到文本")?
            .trim()
            .trim_end_matches('[')
            .to_string();

        Ok(ComicInSearch {
            id,
            title,
            subtitle,
            cover,
            status,
            update_time,
            year,
            region,
            genres,
            authors,
            aliases,
            intro,
        })
    }
}

fn get_id_and_title_and_subtitle(dt: ElementRef) -> anyhow::Result<(i64, String, Option<String>)> {
    let a = dt
        .select(&Selector::parse("dt > a").to_anyhow()?)
        .next()
        .context("没有找到标题和链接的<a>")?;

    let id = a
        .value()
        .attr("href")
        .context("没有在标题和链接的<a>中找到href属性")?
        .trim_start_matches("/comic/")
        .trim_end_matches('/')
        .parse()
        .context("漫画id不是整数")?;

    let title = a
        .value()
        .attr("title")
        .context("没有在标题和链接的<a>中找到title属性")?
        .to_string();

    let subtitle = dt
        .select(&Selector::parse("dt > small > a").to_anyhow()?)
        .next()
        .and_then(|a| a.text().next())
        .map(|text| text.trim().to_string());

    Ok((id, title, subtitle))
}

fn get_status_and_update_time(status_dd: &ElementRef) -> anyhow::Result<(String, String)> {
    let spans = status_dd
        .select(&Selector::parse("span > span").to_anyhow()?)
        .collect::<Vec<_>>();

    let status = spans
        .first()
        .context("没有找到漫画状态的<span>")?
        .text()
        .next()
        .context("没有在漫画状态的<span>中找到文本")?
        .trim()
        .to_string();

    let update_time = spans
        .get(1)
        .context("没有找到更新时间的<span>")?
        .text()
        .next()
        .context("没有在更新时间的<span>中找到文本")?
        .trim()
        .to_string();

    Ok((status, update_time))
}

fn get_year_and_region_and_genres(
    info_dd: &ElementRef,
) -> anyhow::Result<(i64, String, Vec<String>)> {
    let spans = info_dd
        .select(&Selector::parse("span").to_anyhow()?)
        .collect::<Vec<_>>();

    let a_selector = Selector::parse("a").to_anyhow()?;

    let year = spans
        .first()
        .context("没有找到出版年份<span>")?
        .select(&a_selector)
        .next()
        .context("没有找到出版年份<a>")?
        .text()
        .next()
        .context("没有在出版年份<a>中找到文本")?
        .trim()
        .trim_end_matches('年')
        .parse()
        .context("出版年份不是整数")?;

    let region = spans
        .get(1)
        .context("没有找到地区<span>")?
        .select(&a_selector)
        .next()
        .context("没有找到地区<a>")?
        .value()
        .attr("title")
        .context("没有在地区<a>中找到title属性")?
        .to_string();

    let genres = spans
        .get(2)
        .context("没有找到类型<span>")?
        .select(&a_selector)
        .filter_map(|a| a.value().attr("title").map(str::to_string))
        .collect::<Vec<_>>();

    Ok((year, region, genres))
}
