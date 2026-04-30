use tauri::AppHandle;

use crate::{extensions::AppHandleExt, types::Comic};

pub fn filename_filter(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '/' => ' ',
            ':' => '：',
            '*' => '⭐',
            '?' => '？',
            '"' => '\'',
            '<' => '《',
            '>' => '》',
            '|' => '丨',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub async fn get_comic(app: &AppHandle, id: i64) -> anyhow::Result<Comic> {
    let manhuagui_client = app.get_manhuagui_client();

    let comic = manhuagui_client.get_comic(id).await?;

    Ok(comic)
}
