use std::{collections::HashMap};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::{config::Config, extensions::ToAnyhow, utils::filename_filter};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct Comic {
    /// 漫画id
    pub id: i64,
    /// 漫画标题
    pub title: String,
    /// 漫画副标题
    pub subtitle: Option<String>,
    /// 封面链接
    pub cover: String,
    /// 漫画状态(连载中/已完结)
    pub status: String,
    /// 上次更新时间
    pub update_time: String,
    /// 出版年份
    pub year: i64,
    /// 地区
    pub region: String,
    /// 类型
    pub genres: Vec<String>,
    /// 作者
    pub authors: Vec<String>,
    /// 漫画别名
    pub aliases: Vec<String>,
    /// 简介
    pub intro: String,
    /// 组名(单话、单行本...)->章节信息
    pub groups: HashMap<String, Vec<ChapterInfo>>,
}

impl Comic {
    pub fn from_html(app: &AppHandle, html: &str) -> anyhow::Result<Comic> {
        let document = Html::parse_document(html);

        let hidden_fragment = match document
            .select(&Selector::parse("#__VIEWSTATE").to_anyhow()?)
            .next()
        {
            Some(hidden_input) => {
                // 有隐藏数据
                let compressed_data = hidden_input
                    .value()
                    .attr("value")
                    .context("没有在包含隐藏数据的<input>中找到value属性")?;

                let decompressed_data = lz_str::decompress_from_base64(compressed_data)
                    .context("lzstring解压缩失败")?;

                let hidden_html = String::from_utf16(&decompressed_data)
                    .context("lzstring解压缩后的数据不是utf-16字符串")?;

                Some(Html::parse_fragment(&hidden_html))
            }
            None => None,
        };

        let book_detail_div = document
            .select(&Selector::parse(".book-detail").to_anyhow()?)
            .next()
            .context("没有找到漫画详情的<div>")?;

        let id = document
            .select(&Selector::parse(".crumb > a:nth-last-child(1)").to_anyhow()?)
            .next()
            .context("没有找到漫画链接的<a>")?
            .value()
            .attr("href")
            .context("没有在漫画链接的<a>中找到href属性")?
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .context("漫画id不是整数")?;

        let (title, subtitle) = get_title_and_subtitle(&book_detail_div)?;

        let cover_src = document
            .select(&Selector::parse(".hcover img").to_anyhow()?)
            .next()
            .context("没有找到封面的<img>")?
            .value()
            .attr("src")
            .context("没有在封面的<img>中找到src属性")?
            .to_string();
        let cover = format!("https:{cover_src}");

        let detail_lis = book_detail_div
            .select(&Selector::parse(".detail-list > li").to_anyhow()?)
            .collect::<Vec<_>>();

        let li = detail_lis.first().context("没有找到出版年份和地区的<li>")?;
        let (year, region) = get_year_and_region(li)?;

        let li = detail_lis.get(1).context("没有找到漫画类型和作者的<li>")?;
        let (genres, authors) = get_genres_and_authors(li)?;

        let li = detail_lis.get(2).context("没有找到别名的<li>")?;
        let aliases = li
            .select(&Selector::parse("span > a").to_anyhow()?)
            .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
            .collect::<Vec<_>>();

        let li = detail_lis.get(3).context("没有找到状态和更新时间的<li>")?;
        let (status, update_time) = get_status_and_update_time(li)?;

        let intro = book_detail_div
            .select(&Selector::parse("#intro-cut").to_anyhow()?)
            .next()
            .context("没有找到简介的<div>")?
            .text()
            .next()
            .context("没有在简介的<div>中找到文本")?
            .trim()
            .to_string();

        let groups = if let Some(fragment) = hidden_fragment {
            get_groups(app, &fragment.root_element(), id, &title, &status)?
        } else {
            let chapter_div = document
                .select(&Selector::parse(".chapter").to_anyhow()?)
                .next()
                .context("没有找到章节列表的<div>")?;

            get_groups(app, &chapter_div, id, &title, &status)?
        };

        Ok(Comic {
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
            groups,
        })
    }
}

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

fn get_title_and_subtitle(
    book_detail_div: &ElementRef,
) -> anyhow::Result<(String, Option<String>)> {
    let title = book_detail_div
        .select(&Selector::parse(".book-title h1").to_anyhow()?)
        .next()
        .context("没有找到漫画标题的<h1>")?
        .text()
        .next()
        .context("没有在漫画标题的<h1>中找到文本")?
        .trim()
        .to_string();
    let title = filename_filter(&title);

    let subtitle = book_detail_div
        .select(&Selector::parse(".book-title h2").to_anyhow()?)
        .next()
        .and_then(|h2| h2.text().next())
        .map(|text| text.trim().to_string());

    Ok((title, subtitle))
}

fn get_year_and_region(li: &ElementRef) -> anyhow::Result<(i64, String)> {
    let spans = li
        .select(&Selector::parse("span").to_anyhow()?)
        .collect::<Vec<_>>();
    let a_selector = Selector::parse("a").to_anyhow()?;

    let year = spans
        .first()
        .context("没有找到出版年份的<span>")?
        .select(&a_selector)
        .next()
        .context("没有找到出版年份的<a>")?
        .text()
        .next()
        .context("没有在出版年份的<a>中找到文本")?
        .trim()
        .trim_end_matches('年')
        .parse::<i64>()
        .context("出版年份不是整数")?;

    let region = spans
        .get(1)
        .context("没有找到地区的<span>")?
        .select(&a_selector)
        .next()
        .context("没有找到地区的<a>")?
        .value()
        .attr("title")
        .context("没有在地区的<a>中找到title属性")?
        .to_string();

    Ok((year, region))
}

fn get_genres_and_authors(li: &ElementRef) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let spans = li
        .select(&Selector::parse("span").to_anyhow()?)
        .collect::<Vec<_>>();
    let a_selector = Selector::parse("a").to_anyhow()?;

    let genres = spans
        .first()
        .context("没有找到漫画类型的<span>")?
        .select(&a_selector)
        .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
        .collect::<Vec<_>>();

    let authors = spans
        .get(1)
        .context("没有找到作者的<span>")?
        .select(&a_selector)
        .filter_map(|a| a.value().attr("title").map(str::to_string))
        .collect::<Vec<_>>();

    Ok((genres, authors))
}

fn get_status_and_update_time(li: &ElementRef) -> anyhow::Result<(String, String)> {
    let spans = li
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

#[allow(clippy::cast_possible_wrap)]
fn get_groups(
    app: &AppHandle,
    chapter_div: &ElementRef,
    comic_id: i64,
    comic_title: &str,
    comic_status: &str,
) -> anyhow::Result<HashMap<String, Vec<ChapterInfo>>> {
    let h4s = chapter_div
        .select(&Selector::parse("h4").to_anyhow()?)
        .collect::<Vec<_>>();

    let chapter_divs = chapter_div
        .select(&Selector::parse(".chapter-list").to_anyhow()?)
        .collect::<Vec<_>>();

    if h4s.len() != chapter_divs.len() {
        return Err(anyhow!("章节组名和章节列表数量不一致"));
    }

    let mut groups = HashMap::new();
    for (h4, chapter_list_div) in h4s.iter().zip(chapter_divs.iter()) {
        let group_name = h4
            .text()
            .next()
            .context("没有在章节组名的<h4>中找到文本")?
            .trim()
            .to_string();
        let group_name = filename_filter(&group_name);

        let uls = chapter_list_div
            .select(&Selector::parse("ul").to_anyhow()?)
            .collect::<Vec<_>>();

        let mut order = 0.0;
        // 统计一共有多少个li
        let group_size = chapter_list_div
            .select(&Selector::parse("li").to_anyhow()?)
            .count() as i64;

        let mut chapter_infos = Vec::new();
        for ul in uls {
            let mut lis = ul
                .select(&Selector::parse("li").to_anyhow()?)
                .collect::<Vec<_>>();
            lis.reverse();

            for li in lis {
                order += 1.0;
                let a = li
                    .select(&Selector::parse("a").to_anyhow()?)
                    .next()
                    .context("没有找到章节的<a>")?;

                let chapter_id = a
                    .value()
                    .attr("href")
                    .context("没有在章节的<a>中找到href属性")?
                    .trim_start_matches(&format!("/comic/{comic_id}/"))
                    .trim_end_matches(".html")
                    .parse::<i64>()
                    .context("章节id不是整数")?;

                let chapter_title = a
                    .value()
                    .attr("title")
                    .context("没有在章节的<a>中找到title属性")?
                    .to_string();
                let chapter_title = filename_filter(&chapter_title);

                let prefixed_chapter_title = format!("{order} {chapter_title}");

                let chapter_size = a
                    .select(&Selector::parse("span > i").to_anyhow()?)
                    .next()
                    .context("没有找到章节的<i>")?
                    .text()
                    .next()
                    .context("没有在章节的<i>中找到文本")?
                    .trim()
                    .trim_end_matches('p')
                    .parse::<i64>()
                    .context("章节页数不是整数")?;

                let is_downloaded =
                    get_is_downloaded(app, comic_title, &group_name, &prefixed_chapter_title);

                chapter_infos.push(ChapterInfo {
                    chapter_id,
                    chapter_title,
                    chapter_size,
                    prefixed_chapter_title,
                    comic_id,
                    comic_title: comic_title.to_string(),
                    group_name: group_name.clone(),
                    group_size,
                    order,
                    comic_status: comic_status.to_string(),
                    is_downloaded: Some(is_downloaded),
                });
            }
        }

        groups.insert(group_name, chapter_infos);
    }

    Ok(groups)
}

fn get_is_downloaded(
    app: &AppHandle,
    comic_title: &str,
    group_name: &str,
    prefixed_chapter_title: &str,
) -> bool {
    app.state::<RwLock<Config>>()
        .read()
        .download_dir
        .join(comic_title)
        .join(group_name)
        .join(prefixed_chapter_title)
        .exists()
}
