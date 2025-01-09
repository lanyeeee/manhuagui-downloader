use crate::extensions::ToAnyhow;
use anyhow::Context;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub username: String,
    pub avatar: String,
}
impl UserProfile {
    pub fn from_html(html: &str) -> anyhow::Result<UserProfile> {
        let document = Html::parse_document(html);
        // 获取 `.avatar-box` 的 `<div>`
        let avatar_box = document
            .select(&Selector::parse(".avatar-box").to_anyhow()?)
            .next()
            .context("没有找到`.avatar-box`的<div>")?;

        let username = avatar_box
            .select(&Selector::parse("h3").to_anyhow()?)
            .next()
            .map(|h3| h3.text().collect::<String>().trim().to_string())
            .context("没有找到用户名相关的<h3>")?;

        let avatar = avatar_box
            .select(&Selector::parse(".img-box img").to_anyhow()?)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(|src| format!("https:{src}"))
            .context("没有找到头像相关的<img>")?;

        let user_profile = UserProfile { username, avatar };
        Ok(user_profile)
    }
}
