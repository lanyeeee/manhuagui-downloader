use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use eyre::{eyre, OptionExt, WrapErr};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use walkdir::WalkDir;

use crate::{
    extensions::{AppHandleExt, ToEyre, WalkDirEntryExt},
    types::{ChapterDirFmtParams, ChapterInfo},
    utils::{self, filename_filter},
};

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
    /// 是否已下载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comic_download_dir: Option<PathBuf>,
}

impl Comic {
    #[allow(clippy::too_many_lines)]
    pub fn from_html(app: &AppHandle, html: &str) -> eyre::Result<Comic> {
        let document = Html::parse_document(html);

        let hidden_fragment = match document
            .select(&Selector::parse("#__VIEWSTATE").to_eyre()?)
            .next()
        {
            Some(hidden_input) => {
                // 有隐藏数据
                let compressed_data = hidden_input
                    .value()
                    .attr("value")
                    .ok_or_eyre("没有在包含隐藏数据的<input>中找到value属性")?;

                let decompressed_data = lz_str::decompress_from_base64(compressed_data)
                    .ok_or_eyre("lzstring解压缩失败")?;

                let hidden_html = String::from_utf16(&decompressed_data)
                    .wrap_err("lzstring解压缩后的数据不是utf-16字符串")?;

                Some(Html::parse_fragment(&hidden_html))
            }
            None => None,
        };

        let book_detail_div = document
            .select(&Selector::parse(".book-detail").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到漫画详情的<div>")?;

        let id = document
            .select(&Selector::parse(".crumb > a:nth-last-child(1)").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到漫画链接的<a>")?
            .value()
            .attr("href")
            .ok_or_eyre("没有在漫画链接的<a>中找到href属性")?
            .trim_start_matches("/comic/")
            .trim_end_matches('/')
            .parse::<i64>()
            .wrap_err("漫画id不是整数")?;

        let (title, subtitle) = get_title_and_subtitle(&book_detail_div)?;

        let cover_src = document
            .select(&Selector::parse(".hcover img").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到封面的<img>")?
            .value()
            .attr("src")
            .ok_or_eyre("没有在封面的<img>中找到src属性")?
            .to_string();
        let cover = format!("https:{cover_src}");

        let detail_lis = book_detail_div
            .select(&Selector::parse(".detail-list > li").to_eyre()?)
            .collect::<Vec<_>>();

        let li = detail_lis
            .first()
            .ok_or_eyre("没有找到出版年份和地区的<li>")?;
        let (year, region) = get_year_and_region(li)?;

        let li = detail_lis
            .get(1)
            .ok_or_eyre("没有找到漫画类型和作者的<li>")?;
        let (genres, authors) = get_genres_and_authors(li)?;

        let li = detail_lis.get(2).ok_or_eyre("没有找到别名的<li>")?;
        let aliases = li
            .select(&Selector::parse("span > a").to_eyre()?)
            .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
            .collect::<Vec<_>>();

        let li = detail_lis
            .get(3)
            .ok_or_eyre("没有找到状态和更新时间的<li>")?;
        let (status, update_time) = get_status_and_update_time(li)?;

        let intro = book_detail_div
            .select(&Selector::parse("#intro-cut").to_eyre()?)
            .next()
            .ok_or_eyre("没有找到简介的<div>")?
            .text()
            .next()
            .ok_or_eyre("没有在简介的<div>中找到文本")?
            .trim()
            .to_string();

        let groups = if let Some(fragment) = hidden_fragment {
            get_groups(&fragment.root_element(), id, &title, &status)?
        } else {
            let chapter_div = document
                .select(&Selector::parse(".chapter").to_eyre()?)
                .next()
                .ok_or_eyre("没有找到章节列表的<div>")?;

            get_groups(&chapter_div, id, &title, &status)?
        };

        let mut comic = Comic {
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
            is_downloaded: None,
            comic_download_dir: None,
        };

        let id_to_dir_map =
            utils::create_id_to_dir_map(app).wrap_err("创建漫画路径词到下载目录映射失败")?;

        // TODO: 这是为了兼容v0.4.2及之前的版本，后续需要移除，计划在v0.6.0之后移除
        if let Some(comic_download_dir) = id_to_dir_map.get(&comic.id) {
            comic
                .create_chapter_metadata_for_old_version(comic_download_dir)
                .wrap_err("为旧版本创建章节元数据失败")?;
        }

        comic
            .update_fields(&id_to_dir_map)
            .wrap_err(format!("`{}`更新Comic的字段失败", comic.title))?;

        Ok(comic)
    }

    pub fn from_metadata(metadata_path: &Path) -> eyre::Result<Comic> {
        let comic_json = std::fs::read_to_string(metadata_path).wrap_err(format!(
            "从元数据转为Comic失败，读取元数据文件`{}`失败",
            metadata_path.display()
        ))?;
        let mut comic = serde_json::from_str::<Comic>(&comic_json).wrap_err(format!(
            "从元数据转为Comic失败，将`{}`反序列化为Comic失败",
            metadata_path.display()
        ))?;
        let parent = metadata_path
            .parent()
            .ok_or_eyre(format!("`{}`没有父目录", metadata_path.display()))?;
        let comic_download_dir = parent.to_path_buf();

        // TODO: 这是为了兼容v0.4.2及之前的版本，后续需要移除，计划在v0.6.0之后移除
        comic
            .create_chapter_metadata_for_old_version(&comic_download_dir)
            .wrap_err("为旧版本创建章节元数据失败")?;

        comic.comic_download_dir = Some(comic_download_dir);
        comic.is_downloaded = Some(true);

        // 来自元数据的章节信息没有`chapter_download_dir`和`is_downloaded`字段，需要更新
        comic
            .update_chapter_infos_fields()
            .wrap_err("更新章节信息字段失败")?;

        Ok(comic)
    }

    pub fn update_fields(&mut self, id_to_dir_map: &HashMap<i64, PathBuf>) -> eyre::Result<()> {
        if let Some(comic_download_dir) = id_to_dir_map.get(&self.id) {
            self.comic_download_dir = Some(comic_download_dir.clone());
            self.is_downloaded = Some(true);

            self.update_chapter_infos_fields()
                .wrap_err("更新章节信息字段失败")?;
        }

        Ok(())
    }

    fn update_chapter_infos_fields(&mut self) -> eyre::Result<()> {
        let Some(comic_download_dir) = &self.comic_download_dir else {
            return Err(eyre!("`comic_download_dir`字段为`None`"));
        };

        if !comic_download_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(comic_download_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.is_chapter_metadata() {
                continue;
            }

            let metadata_path = entry.path();

            let metadata_str = std::fs::read_to_string(metadata_path)
                .wrap_err(format!("读取`{}`失败", metadata_path.display()))?;

            let chapter_json: serde_json::Value =
                serde_json::from_str(&metadata_str).wrap_err(format!(
                    "将`{}`反序列化为serde_json::Value失败",
                    metadata_path.display()
                ))?;

            let chapter_id = chapter_json
                .get("chapterId")
                .and_then(serde_json::Value::as_i64)
                .ok_or_eyre(format!("`{}`没有`chapterId`字段", metadata_path.display()))?;

            let group_name = chapter_json
                .get("groupName")
                .and_then(|word| word.as_str())
                .ok_or_eyre(format!("`{}`没有`groupName`字段", metadata_path.display()))?
                .to_string();

            let Some(group) = self.groups.get_mut(&group_name) else {
                continue;
            };

            if let Some(chapter_info) = group
                .iter_mut()
                .find(|chapter| chapter.chapter_id == chapter_id)
            {
                let parent = metadata_path
                    .parent()
                    .ok_or_eyre(format!("`{}`没有父目录", metadata_path.display()))?;
                chapter_info.chapter_download_dir = Some(parent.to_path_buf());
                chapter_info.is_downloaded = Some(true);
            }
        }

        Ok(())
    }

    pub fn save_metadata(&self) -> eyre::Result<()> {
        let mut comic = self.clone();
        // 将所有的is_downloaded字段设置为None，这样能使is_downloaded字段在序列化时被忽略
        comic.is_downloaded = None;
        for chapter_infos in comic.groups.values_mut() {
            for chapter_info in chapter_infos.iter_mut() {
                chapter_info.is_downloaded = None;
            }
        }

        let comic_download_dir = self
            .comic_download_dir
            .as_ref()
            .ok_or_eyre("`comic_download_dir`字段为`None`")?;
        let metadata_path = comic_download_dir.join("元数据.json");

        std::fs::create_dir_all(comic_download_dir)
            .wrap_err(format!("创建目录`{}`失败", comic_download_dir.display()))?;

        let comic_json =
            serde_json::to_string_pretty(&comic).wrap_err("将Comic序列化为json失败")?;

        std::fs::write(&metadata_path, comic_json)
            .wrap_err(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    pub fn get_comic_export_dir(&self, app: &AppHandle) -> eyre::Result<PathBuf> {
        let (download_dir, export_dir) = {
            let config = app.get_config();
            let config = config.read();
            (config.download_dir.clone(), config.export_dir.clone())
        };

        let Some(comic_download_dir) = self.comic_download_dir.clone() else {
            return Err(eyre!("`comic_download_dir`字段为`None`"));
        };

        let relative_dir = comic_download_dir
            .strip_prefix(&download_dir)
            .wrap_err(format!(
                "无法从路径`{}`中移除前缀`{}`",
                comic_download_dir.display(),
                download_dir.display()
            ))?;

        let comic_export_dir = export_dir.join(relative_dir);
        Ok(comic_export_dir)
    }

    pub fn ensure_download_dir_fields(&mut self, app: &AppHandle) -> eyre::Result<()> {
        if self.has_download_dir_fields() {
            return Ok(());
        }

        self.update_download_dir_fields_by_fmt(app)
    }

    pub fn has_download_dir_fields(&self) -> bool {
        let comic_download_dir_ready = self.comic_download_dir.is_some();
        let chapter_download_dir_ready = self
            .groups
            .values()
            .flatten()
            .all(|chapter| chapter.chapter_download_dir.is_some());

        comic_download_dir_ready && chapter_download_dir_ready
    }

    /// 根据fmt更新`comic_download_dir`和`chapter_infos.chapter_download_dir`字段
    pub fn update_download_dir_fields_by_fmt(&mut self, app: &AppHandle) -> eyre::Result<()> {
        let comic_id = self.id;
        let comic_title = self.title.clone();
        let comic_subtitle = self.subtitle.clone().unwrap_or_default();
        let author = self.authors.join(", ");

        let comic_dir_fmt_params = ComicDirFmtParams {
            comic_id,
            comic_title: comic_title.clone(),
            comic_subtitle: comic_subtitle.clone(),
            pub_year: self.year,
            region: self.region.clone(),
            author: author.clone(),
        };
        let comic_download_dir = Comic::get_comic_download_dir_by_fmt(app, &comic_dir_fmt_params)?;
        self.comic_download_dir = Some(comic_download_dir.clone());

        for chapter_info in &mut self.groups.values_mut().flatten() {
            let chapter_dir_fmt_params = ChapterDirFmtParams {
                comic_id,
                comic_title: comic_title.clone(),
                comic_subtitle: comic_subtitle.clone(),
                pub_year: self.year,
                region: self.region.clone(),
                author: author.clone(),
                group_name: chapter_info.group_name.clone(),
                chapter_id: chapter_info.chapter_id,
                chapter_title: chapter_info.chapter_title.clone(),
                order: chapter_info.order,
            };
            let chapter_download_dir = ChapterInfo::get_chapter_download_dir_by_fmt(
                app,
                &comic_download_dir,
                &chapter_dir_fmt_params,
            )?;
            chapter_info.chapter_download_dir = Some(chapter_download_dir);
        }

        Ok(())
    }

    pub fn get_comic_download_dir_by_fmt(
        app: &AppHandle,
        fmt_params: &ComicDirFmtParams,
    ) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .wrap_err("将ComicDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("ComicDirFmtParams不是JSON对象")?;

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

        let (download_dir, comic_dir_fmt) = {
            let config = app.get_config();
            let config = config.read();
            (config.download_dir.clone(), config.comic_dir_fmt.clone())
        };

        let dir_fmt_parts: Vec<&str> = comic_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).wrap_err("格式化目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }

        let mut comic_download_dir = download_dir;
        for dir_name in dir_names {
            comic_download_dir = comic_download_dir.join(dir_name);
        }

        Ok(comic_download_dir)
    }

    fn create_chapter_metadata_for_old_version(
        &self,
        comic_download_dir: &Path,
    ) -> eyre::Result<()> {
        let mut chapter_dirs = HashSet::new();
        for group_entry in std::fs::read_dir(comic_download_dir)?.filter_map(Result::ok) {
            let Ok(file_type) = group_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            for chapter_entry in std::fs::read_dir(group_entry.path())?.filter_map(Result::ok) {
                let Ok(file_type) = chapter_entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                chapter_dirs.insert(chapter_entry.path());
            }
        }

        for chapter_info in self.groups.values().flatten() {
            let group_title = filename_filter(&chapter_info.group_name);
            let chapter_title = filename_filter(&chapter_info.chapter_title);
            let order = chapter_info.order;
            let prefixed_chapter_title = format!("{order} {chapter_title}");

            let old_chapter_dir = comic_download_dir
                .join(&group_title)
                .join(&prefixed_chapter_title);

            let old_chapter_dir_exists = chapter_dirs.contains(&old_chapter_dir);
            let old_chapter_metadata_exists = old_chapter_dir.join("章节元数据.json").exists();

            if old_chapter_dir_exists && !old_chapter_metadata_exists {
                // 如果旧版本的章节目录存在，但没有元数据文件，就创建一个
                let mut info = chapter_info.clone();
                info.chapter_download_dir = Some(old_chapter_dir);
                info.is_downloaded = Some(true);
                info.save_metadata()?;
            }
        }

        Ok(())
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize, Type)]
pub struct ComicDirFmtParams {
    pub comic_id: i64,
    pub comic_title: String,
    pub comic_subtitle: String,
    pub pub_year: i64,
    pub region: String,
    pub author: String,
}

fn get_title_and_subtitle(book_detail_div: &ElementRef) -> eyre::Result<(String, Option<String>)> {
    let title = book_detail_div
        .select(&Selector::parse(".book-title h1").to_eyre()?)
        .next()
        .ok_or_eyre("没有找到漫画标题的<h1>")?
        .text()
        .next()
        .ok_or_eyre("没有在漫画标题的<h1>中找到文本")?
        .trim()
        .to_string();
    let title = filename_filter(&title);

    let subtitle = book_detail_div
        .select(&Selector::parse(".book-title h2").to_eyre()?)
        .next()
        .and_then(|h2| h2.text().next())
        .map(|text| text.trim().to_string());

    Ok((title, subtitle))
}

fn get_year_and_region(li: &ElementRef) -> eyre::Result<(i64, String)> {
    let spans = li
        .select(&Selector::parse("span").to_eyre()?)
        .collect::<Vec<_>>();
    let a_selector = Selector::parse("a").to_eyre()?;

    let year = spans
        .first()
        .ok_or_eyre("没有找到出版年份的<span>")?
        .select(&a_selector)
        .next()
        .ok_or_eyre("没有找到出版年份的<a>")?
        .text()
        .next()
        .ok_or_eyre("没有在出版年份的<a>中找到文本")?
        .trim()
        .trim_end_matches('年')
        .parse::<i64>()
        .wrap_err("出版年份不是整数")?;

    let region = spans
        .get(1)
        .ok_or_eyre("没有找到地区的<span>")?
        .select(&a_selector)
        .next()
        .ok_or_eyre("没有找到地区的<a>")?
        .value()
        .attr("title")
        .ok_or_eyre("没有在地区的<a>中找到title属性")?
        .to_string();

    Ok((year, region))
}

fn get_genres_and_authors(li: &ElementRef) -> eyre::Result<(Vec<String>, Vec<String>)> {
    let spans = li
        .select(&Selector::parse("span").to_eyre()?)
        .collect::<Vec<_>>();
    let a_selector = Selector::parse("a").to_eyre()?;

    let genres = spans
        .first()
        .ok_or_eyre("没有找到漫画类型的<span>")?
        .select(&a_selector)
        .filter_map(|a| a.text().next().map(|text| text.trim().to_string()))
        .collect::<Vec<_>>();

    let authors = spans
        .get(1)
        .ok_or_eyre("没有找到作者的<span>")?
        .select(&a_selector)
        .filter_map(|a| a.value().attr("title").map(str::to_string))
        .collect::<Vec<_>>();

    Ok((genres, authors))
}

fn get_status_and_update_time(li: &ElementRef) -> eyre::Result<(String, String)> {
    let spans = li
        .select(&Selector::parse("span > span").to_eyre()?)
        .collect::<Vec<_>>();

    let status = spans
        .first()
        .ok_or_eyre("没有找到漫画状态的<span>")?
        .text()
        .next()
        .ok_or_eyre("没有在漫画状态的<span>中找到文本")?
        .trim()
        .to_string();

    let update_time = spans
        .get(1)
        .ok_or_eyre("没有找到更新时间的<span>")?
        .text()
        .next()
        .ok_or_eyre("没有在更新时间的<span>中找到文本")?
        .trim()
        .to_string();

    Ok((status, update_time))
}

#[allow(clippy::cast_possible_wrap)]
fn get_groups(
    chapter_div: &ElementRef,
    comic_id: i64,
    comic_title: &str,
    comic_status: &str,
) -> eyre::Result<HashMap<String, Vec<ChapterInfo>>> {
    let mut group_names = chapter_div
        .select(&Selector::parse("h4").to_eyre()?)
        .map(|h4| h4.text().next().unwrap_or_default().trim().to_string())
        .collect::<Vec<_>>();

    let chapter_divs = chapter_div
        .select(&Selector::parse(".chapter-list").to_eyre()?)
        .collect::<Vec<_>>();

    // 保证 group_names.len() == chapter_divs.len()
    while group_names.len() < chapter_divs.len() {
        group_names.push(String::new());
    }

    let empty_count = group_names.iter().filter(|s| s.is_empty()).count();
    let mut empty_index = 0;

    let mut group_name_and_chapter_divs = group_names
        .into_iter()
        .zip(chapter_divs)
        .collect::<Vec<_>>();

    for (group_name, _) in &mut group_name_and_chapter_divs {
        if !group_name.is_empty() {
            continue;
        }
        // 处理没有group_name的情况
        empty_index += 1;
        if empty_count == 1 {
            *group_name = "其他".to_string();
        } else {
            *group_name = format!("其他{empty_index}");
        }
    }

    let mut groups = HashMap::new();
    for (group_name, chapter_list_div) in group_name_and_chapter_divs {
        let group_name = filename_filter(&group_name);

        let uls = chapter_list_div
            .select(&Selector::parse("ul").to_eyre()?)
            .collect::<Vec<_>>();

        let mut order = 0.0;
        // 统计一共有多少个li
        let group_size = chapter_list_div
            .select(&Selector::parse("li").to_eyre()?)
            .count() as i64;

        let mut chapter_infos = Vec::new();
        for ul in uls {
            let mut lis = ul
                .select(&Selector::parse("li").to_eyre()?)
                .collect::<Vec<_>>();
            lis.reverse();

            for li in lis {
                order += 1.0;
                let a = li
                    .select(&Selector::parse("a").to_eyre()?)
                    .next()
                    .ok_or_eyre("没有找到章节的<a>")?;

                let chapter_id = a
                    .value()
                    .attr("href")
                    .ok_or_eyre("没有在章节的<a>中找到href属性")?
                    .trim_start_matches(&format!("/comic/{comic_id}/"))
                    .trim_end_matches(".html")
                    .parse::<i64>()
                    .wrap_err("章节id不是整数")?;

                let chapter_title = a
                    .value()
                    .attr("title")
                    .ok_or_eyre("没有在章节的<a>中找到title属性")?
                    .to_string();
                let chapter_title = filename_filter(&chapter_title);

                let chapter_size = a
                    .select(&Selector::parse("span > i").to_eyre()?)
                    .next()
                    .ok_or_eyre("没有找到章节的<i>")?
                    .text()
                    .next()
                    .ok_or_eyre("没有在章节的<i>中找到文本")?
                    .trim()
                    .trim_end_matches('p')
                    .parse::<i64>()
                    .wrap_err("章节页数不是整数")?;

                chapter_infos.push(ChapterInfo {
                    chapter_id,
                    chapter_title,
                    chapter_size,
                    comic_id,
                    comic_title: comic_title.to_string(),
                    group_name: group_name.clone(),
                    group_size,
                    order,
                    comic_status: comic_status.to_string(),
                    is_downloaded: None,
                    chapter_download_dir: None,
                });
            }
        }

        groups.insert(group_name, chapter_infos);
    }

    Ok(groups)
}
