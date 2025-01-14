use anyhow::Context;
use parking_lot::RwLock;
use tauri::{AppHandle, State};

use crate::{
    config::Config,
    download_manager::DownloadManager,
    errors::CommandResult,
    manhuagui_client::ManhuaguiClient,
    types::{ChapterInfo, Comic, GetFavoriteResult, SearchResult, UserProfile},
};

#[tauri::command]
#[specta::specta]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_config(config: tauri::State<RwLock<Config>>) -> Config {
    config.read().clone()
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn save_config(
    app: AppHandle,
    config_state: State<RwLock<Config>>,
    config: Config,
) -> CommandResult<()> {
    let mut config_state = config_state.write();
    *config_state = config;
    config_state.save(&app)?;
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn login(
    manhuagui_client: State<'_, ManhuaguiClient>,
    username: String,
    password: String,
) -> CommandResult<String> {
    let cookie = manhuagui_client
        .login(&username, &password)
        .await
        .context("使用账号密码登录失败")?;
    Ok(cookie)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_user_profile(
    manhuagui_client: State<'_, ManhuaguiClient>,
) -> CommandResult<UserProfile> {
    let user_profile = manhuagui_client
        .get_user_profile()
        .await
        .context("获取用户信息失败")?;
    Ok(user_profile)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn search(
    manhuagui_client: State<'_, ManhuaguiClient>,
    keyword: String,
    page_num: i64,
) -> CommandResult<SearchResult> {
    let search_result = manhuagui_client
        .search(&keyword, page_num)
        .await
        .context("搜索失败")?;
    Ok(search_result)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_comic(
    manhuagui_client: State<'_, ManhuaguiClient>,
    id: i64,
) -> CommandResult<Comic> {
    let comic = manhuagui_client
        .get_comic(id)
        .await
        .context(format!("获取漫画`{id}`的信息失败"))?;
    Ok(comic)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn download_chapters(
    download_manager: State<'_, DownloadManager>,
    chapters: Vec<ChapterInfo>,
) -> CommandResult<()> {
    for ep in chapters {
        download_manager.submit_chapter(ep).await?;
    }
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_favorite(
    manhuagui_client: State<'_, ManhuaguiClient>,
    page_num: i64,
) -> CommandResult<GetFavoriteResult> {
    let get_favorite_result = manhuagui_client
        .get_favorite(page_num)
        .await
        .context("获取收藏夹失败")?;
    Ok(get_favorite_result)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn save_metadata(config: State<RwLock<Config>>, mut comic: Comic) -> CommandResult<()> {
    // 将所有章节的is_downloaded字段设置为None，这样能使is_downloaded字段在序列化时被忽略
    for chapter_infos in comic.groups.values_mut() {
        for chapter_info in chapter_infos.iter_mut() {
            chapter_info.is_downloaded = None;
        }
    }

    let comic_title = &comic.title;
    let comic_json = serde_json::to_string_pretty(&comic).context(format!(
        "`{comic_title}`的元数据保存失败，将Comic序列化为json失败"
    ))?;

    let download_dir = config.read().download_dir.clone();
    let metadata_dir = download_dir.join(comic_title);
    let metadata_path = metadata_dir.join("元数据.json");

    std::fs::create_dir_all(&metadata_dir).context(format!(
        "`{comic_title}`的元数据保存失败，创建目录`{metadata_dir:?}`失败"
    ))?;

    std::fs::write(&metadata_path, comic_json).context(format!(
        "`{comic_title}`的元数据保存失败，写入文件`{metadata_path:?}`失败"
    ))?;

    Ok(())
}
