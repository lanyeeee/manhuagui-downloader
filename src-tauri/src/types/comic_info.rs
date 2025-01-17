use serde::{Deserialize, Serialize};
use specta::Type;
use yaserde::{YaDeserialize, YaSerialize};

use super::ChapterInfo;

/// 主要参考了[Kavita的文档](https://wiki.kavitareader.com/guides/metadata/comics/)
#[derive(
    Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type, YaSerialize, YaDeserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ComicInfo {
    #[yaserde(rename = "Manga")]
    pub manga: String,
    /// 漫画名
    #[yaserde(rename = "Series")]
    pub series: String,
    /// 出版社
    #[yaserde(rename = "Publisher")]
    pub publisher: String,
    /// 作者
    #[yaserde(rename = "Writer")]
    pub writer: String,
    /// 漫画类型
    #[yaserde(rename = "Genre")]
    pub genre: String,
    #[yaserde(rename = "Summary")]
    pub summary: String,
    /// 章节名
    #[yaserde(rename = "Title")]
    pub title: String,
    /// 普通章节序号
    #[yaserde(rename = "Number")]
    pub number: Option<String>,
    /// 卷序号
    #[yaserde(rename = "Volume")]
    pub volume: Option<String>,
    /// 如果值为Special，则该章节会被Kavita视为特刊
    #[yaserde(rename = "Format")]
    pub format: Option<String>,
    /// 该章节的有多少页
    #[yaserde(rename = "PageCount")]
    pub page_count: i64,
    /// 章节总数
    /// - `0` => Ongoing  
    /// - `非零`且与`Number`或`Volume`一致 => Completed  
    /// - `其他非零值` => Ended
    #[yaserde(rename = "Count")]
    pub count: i64,
}
impl ComicInfo {
    #[allow(clippy::cast_possible_wrap)]
    pub fn from(
        chapter_info: ChapterInfo,
        authors: &[String],
        genre: &[String],
        intro: String,
    ) -> ComicInfo {
        let order = Some(chapter_info.order.to_string());
        let (number, volume, format) = match chapter_info.group_name.as_str() {
            "单话" => (order, None, None),
            "单行本" => (None, order, None),
            _ => (order, None, Some("Special".to_string())),
        };

        let count = match chapter_info.comic_status.as_ref() {
            "连载中" => 0,
            _ => chapter_info.group_size,
        };

        ComicInfo {
            manga: "Yes".to_string(),
            series: chapter_info.comic_title,
            publisher: "漫画柜".to_string(),
            writer: authors.join(", "),
            genre: genre.join(", "),
            summary: intro,
            title: chapter_info.chapter_title,
            number,
            volume,
            format,
            page_count: chapter_info.chapter_size,
            count,
        }
    }
}
