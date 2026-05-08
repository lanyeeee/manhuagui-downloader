use std::{collections::HashMap, io::Cursor, path::PathBuf};

use anyhow::Context;
use image::ImageReader;
use tauri::AppHandle;
use walkdir::WalkDir;

use crate::{
    extensions::{AppHandleExt, WalkDirEntryExt},
    types::Comic,
};

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

pub fn create_id_to_dir_map(app: &AppHandle) -> anyhow::Result<HashMap<i64, PathBuf>> {
    let mut id_to_dir_map: HashMap<i64, PathBuf> = HashMap::new();
    let download_dir = app.get_config().read().download_dir.clone();
    if !download_dir.exists() {
        return Ok(id_to_dir_map);
    }

    for entry in WalkDir::new(&download_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.is_comic_metadata() {
            continue;
        }

        let metadata_str =
            std::fs::read_to_string(path).context(format!("读取`{}`失败", path.display()))?;
        let comic_json: serde_json::Value = serde_json::from_str(&metadata_str).context(
            format!("将`{}`反序列化为serde_json::Value失败", path.display()),
        )?;
        let id = comic_json
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .context(format!("`{}`没有`id`字段", path.display()))?;

        let parent = path
            .parent()
            .context(format!("`{}`没有父目录", path.display()))?;

        id_to_dir_map.entry(id).or_insert(parent.to_path_buf());
    }

    Ok(id_to_dir_map)
}

pub fn get_dimensions(img_data: &[u8]) -> anyhow::Result<(u32, u32)> {
    let reader = ImageReader::new(Cursor::new(&img_data)).with_guessed_format()?;
    let dimensions = reader.into_dimensions()?;
    Ok(dimensions)
}
