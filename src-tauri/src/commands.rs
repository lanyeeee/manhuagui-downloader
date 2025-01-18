use anyhow::Context;
use parking_lot::RwLock;
use tauri::{AppHandle, State};

use crate::{
    config::Config,
    download_manager::DownloadManager,
    errors::CommandResult,
    export,
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

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_downloaded_comics(
    app: AppHandle,
    config: State<RwLock<Config>>,
) -> CommandResult<Vec<Comic>> {
    let download_dir = config.read().download_dir.clone();
    // 遍历下载目录，获取所有元数据文件的路径和修改时间
    let mut metadata_path_with_modify_time = std::fs::read_dir(&download_dir)
        .context(format!(
            "获取已下载的漫画失败，读取下载目录 {download_dir:?} 失败"
        ))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata_path = entry.path().join("元数据.json");
            if !metadata_path.exists() {
                return None;
            }
            let modify_time = metadata_path.metadata().ok()?.modified().ok()?;
            Some((metadata_path, modify_time))
        })
        .collect::<Vec<_>>();
    // 按照文件修改时间排序，最新的排在最前面
    metadata_path_with_modify_time.sort_by(|(_, a), (_, b)| b.cmp(a));
    // 从元数据文件中读取Comic
    let downloaded_comics = metadata_path_with_modify_time
        .iter()
        // TODO: 如果读取元数据失败，应该发送错误Event通知前端，然后才跳过
        .filter_map(|(metadata_path, _)| Comic::from_metadata(&app, metadata_path).ok())
        .collect::<Vec<_>>();

    Ok(downloaded_comics)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_cbz(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.title.clone();
    export::cbz(&app, comic).context(format!("漫画`{comic_title}`导出cbz失败"))?;
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_pdf(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.title.clone();
    export::pdf(&app, comic).context(format!("漫画`{comic_title}`导出pdf失败"))?;
    Ok(())
}
