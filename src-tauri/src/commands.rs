use std::collections::HashMap;

use anyhow::Context;
use parking_lot::RwLock;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

use crate::{
    config::Config,
    download_manager::DownloadManager,
    errors::{CommandError, CommandResult},
    events::UpdateDownloadedComicsEvent,
    export,
    extensions::AnyhowErrorToStringChain,
    logger,
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
    let config = config.read().clone();
    tracing::debug!("获取配置成功");
    config
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn save_config(
    app: AppHandle,
    config_state: State<RwLock<Config>>,
    config: Config,
) -> CommandResult<()> {
    let enable_file_logger = config.enable_file_logger;
    let enable_file_logger_changed = config_state
        .read()
        .enable_file_logger
        .ne(&enable_file_logger);

    let mut config_state = config_state.write();
    *config_state = config;
    config_state
        .save(&app)
        .map_err(|err| CommandError::from("保存配置失败", err))?;
    drop(config_state);
    tracing::debug!("保存配置成功");

    if enable_file_logger_changed {
        if enable_file_logger {
            logger::reload_file_logger()
                .map_err(|err| CommandError::from("重新加载文件日志失败", err))?;
        } else {
            logger::disable_file_logger()
                .map_err(|err| CommandError::from("禁用文件日志失败", err))?;
        }
    }

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
        .map_err(|err| CommandError::from("使用账号密码登录失败", err))?;
    tracing::debug!("登录成功");
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
        .map_err(|err| CommandError::from("获取用户信息失败", err))?;
    tracing::debug!("获取用户信息成功");
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
        .map_err(|err| CommandError::from("搜索失败", err))?;
    tracing::debug!("搜索成功");
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
        .map_err(|err| CommandError::from(&format!("获取漫画`{id}`的信息失败"), err))?;
    tracing::debug!("获取漫画信息成功");
    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn download_chapters(download_manager: State<DownloadManager>, chapters: Vec<ChapterInfo>) {
    for chapter in chapters {
        download_manager.create_download_task(chapter);
    }
    tracing::debug!("下载任务创建成功");
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
        .map_err(|err| CommandError::from("获取收藏夹失败", err))?;
    tracing::debug!("获取收藏夹成功");
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
    let comic_json = serde_json::to_string_pretty(&comic)
        .context("将Comic序列化为json失败")
        .map_err(|err| CommandError::from(&format!("`{comic_title}`的元数据保存失败"), err))?;

    let download_dir = config.read().download_dir.clone();
    let metadata_dir = download_dir.join(comic_title);
    let metadata_path = metadata_dir.join("元数据.json");

    std::fs::create_dir_all(&metadata_dir)
        .context(format!("创建目录`{metadata_dir:?}`失败"))
        .map_err(|err| CommandError::from(&format!("`{comic_title}`的元数据保存失败"), err))?;

    std::fs::write(&metadata_path, comic_json)
        .context(format!("写入文件`{metadata_path:?}`失败"))
        .map_err(|err| CommandError::from(&format!("`{comic_title}`的元数据保存失败"), err))?;

    tracing::debug!("`{comic_title}`的元数据保存成功");
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
        .context(format!("读取下载目录`{download_dir:?}`失败"))
        .map_err(|err| CommandError::from("获取已下载的漫画失败", err))?
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
        .filter_map(|(metadata_path, _)| {
            match Comic::from_metadata(&app, metadata_path).map_err(anyhow::Error::from) {
                Ok(comic) => Some(comic),
                Err(err) => {
                    let err_title = format!("读取元数据文件`{metadata_path:?}`失败");
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    None
                }
            }
        })
        .collect::<Vec<_>>();

    tracing::debug!("获取已下载的漫画成功");
    Ok(downloaded_comics)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_cbz(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.title.clone();
    export::cbz(&app, comic)
        .map_err(|err| CommandError::from(&format!("漫画`{comic_title}`导出cbz失败"), err))?;
    tracing::debug!("漫画`{comic_title}`导出cbz成功");
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_pdf(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.title.clone();
    export::pdf(&app, comic)
        .map_err(|err| CommandError::from(&format!("漫画`{comic_title}`导出pdf失败"), err))?;
    tracing::debug!("漫画`{comic_title}`导出pdf成功");
    Ok(())
}

#[allow(clippy::cast_possible_wrap)]
#[tauri::command(async)]
#[specta::specta]
pub async fn update_downloaded_comics(
    app: AppHandle,
    download_manager: State<'_, DownloadManager>,
) -> CommandResult<()> {
    // 从下载目录中获取已下载的漫画
    let downloaded_comics = get_downloaded_comics(app.clone(), app.state::<RwLock<Config>>())?;
    // 用于存储最新的漫画信息
    let mut latest_comics = Vec::new();
    // 发送正在获取漫画事件
    let total = downloaded_comics.len() as i64;
    let _ = UpdateDownloadedComicsEvent::GettingComics { total }.emit(&app);
    // 获取已下载漫画的最新信息，不用并发是有意为之，防止被封IP
    for (i, downloaded_comic) in downloaded_comics.iter().enumerate() {
        // 获取最新的漫画信息
        let comic = get_comic(app.state::<ManhuaguiClient>(), downloaded_comic.id).await?;
        // 将最新的漫画信息保存到元数据文件
        save_metadata(app.state::<RwLock<Config>>(), comic.clone())?;

        latest_comics.push(comic);
        // 发送获取到漫画事件
        let current = i as i64 + 1;
        let _ = UpdateDownloadedComicsEvent::ComicGot { current, total }.emit(&app);
    }
    // 至此，已下载的漫画的最新信息已获取完毕
    let chapters_to_download = latest_comics
        .into_iter()
        .filter_map(|comic| {
            // 先过滤出每个漫画中至少有一个已下载章节的组
            let downloaded_group = comic
                .groups
                .into_iter()
                .filter_map(|(group_name, chapter_infos)| {
                    // 检查当前组是否有已下载章节，如果有，则返回组路径和章节信息，否则返回None(跳过)
                    chapter_infos
                        .iter()
                        .any(|chapter_info| chapter_info.is_downloaded.unwrap_or(false))
                        .then_some((group_name, chapter_infos))
                })
                .collect::<HashMap<_, _>>();
            // 如果所有组都没有已下载章节，则跳过
            if downloaded_group.is_empty() {
                return None;
            }
            Some(downloaded_group)
        })
        .flat_map(|downloaded_groups| {
            // 从至少有一个已下载章节的组中过滤出其中未下载的章节
            downloaded_groups
                .into_values()
                .flat_map(|chapter_infos| {
                    chapter_infos
                        .into_iter()
                        .filter(|chapter_info| !chapter_info.is_downloaded.unwrap_or(false))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // 下载未下载章节
    download_chapters(download_manager, chapters_to_download);
    // 发送下载任务创建完成事件
    let _ = UpdateDownloadedComicsEvent::DownloadTaskCreated.emit(&app);

    tracing::debug!("为已下载的漫画创建更新任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_logs_dir_size(app: AppHandle) -> CommandResult<u64> {
    let logs_dir = logger::logs_dir(&app)
        .context("获取日志目录失败")
        .map_err(|err| CommandError::from("获取日志目录大小失败", err))?;
    let logs_dir_size = std::fs::read_dir(&logs_dir)
        .context(format!("读取日志目录`{logs_dir:?}`失败"))
        .map_err(|err| CommandError::from("获取日志目录大小失败", err))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum::<u64>();
    tracing::debug!("获取日志目录大小成功");
    Ok(logs_dir_size)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn show_path_in_file_manager(app: AppHandle, path: &str) -> CommandResult<()> {
    app.opener()
        .reveal_item_in_dir(path)
        .context(format!("在文件管理器中打开`{path}`失败"))
        .map_err(|err| CommandError::from("在文件管理器中打开失败", err))?;
    tracing::debug!("在文件管理器中打开成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn pause_download_task(
    download_manager: State<DownloadManager>,
    chapter_id: i64,
) -> CommandResult<()> {
    download_manager
        .pause_download_task(chapter_id)
        .map_err(|err| CommandError::from(&format!("暂停章节ID为`{chapter_id}`的下载任务"), err))?;
    tracing::debug!("暂停章节ID为`{chapter_id}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn resume_download_task(
    download_manager: State<DownloadManager>,
    chapter_id: i64,
) -> CommandResult<()> {
    download_manager
        .resume_download_task(chapter_id)
        .map_err(|err| {
            CommandError::from(&format!("恢复章节ID为`{chapter_id}`的下载任务失败"), err)
        })?;
    tracing::debug!("恢复章节ID为`{chapter_id}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn cancel_download_task(
    download_manager: State<DownloadManager>,
    chapter_id: i64,
) -> CommandResult<()> {
    download_manager
        .cancel_download_task(chapter_id)
        .map_err(|err| {
            CommandError::from(&format!("取消章节ID为`{chapter_id}`的下载任务失败"), err)
        })?;
    tracing::debug!("取消章节ID为`{chapter_id}`的下载任务成功");
    Ok(())
}
