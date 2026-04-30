use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;
use tokio::time::sleep;

use crate::{
    config::Config,
    errors::{CommandError, CommandResult},
    events::UpdateDownloadedComicsEvent,
    export,
    extensions::{AnyhowErrorToStringChain, AppHandleExt},
    logger,
    types::{
        ChapterInfo, Comic, ComicInFavorite, ComicInSearch, GetFavoriteResult, SearchResult,
        UserProfile,
    },
    utils,
};

#[tauri::command]
#[specta::specta]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_config(app: AppHandle) -> Config {
    let config = app.get_config().read().clone();

    tracing::debug!("获取配置成功");
    config
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn save_config(app: AppHandle, config: Config) -> CommandResult<()> {
    let config_state = app.get_config();
    let manhuagui_client = app.get_manhuagui_client();

    let proxy_changed = {
        let config_state = config_state.read();
        config_state.proxy_mode != config.proxy_mode
            || config_state.proxy_host != config.proxy_host
            || config_state.proxy_port != config.proxy_port
    };

    let enable_file_logger = config.enable_file_logger;
    let enable_file_logger_changed = config_state
        .read()
        .enable_file_logger
        .ne(&enable_file_logger);

    {
        // 包裹在大括号中，以便自动释放写锁
        let mut config_state = config_state.write();
        *config_state = config;
        config_state
            .save(&app)
            .map_err(|err| CommandError::from("保存配置失败", err))?;
        tracing::debug!("保存配置成功");
    }

    if proxy_changed {
        manhuagui_client.reload_client();
    }

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
pub async fn login(app: AppHandle, username: String, password: String) -> CommandResult<String> {
    let manhuagui_client = app.get_manhuagui_client();

    let cookie = manhuagui_client
        .login(&username, &password)
        .await
        .map_err(|err| CommandError::from("使用账号密码登录失败", err))?;
    tracing::debug!("登录成功");
    Ok(cookie)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_user_profile(app: AppHandle) -> CommandResult<UserProfile> {
    let manhuagui_client = app.get_manhuagui_client();

    let user_profile = manhuagui_client
        .get_user_profile()
        .await
        .map_err(|err| CommandError::from("获取用户信息失败", err))?;
    tracing::debug!("获取用户信息成功");
    Ok(user_profile)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn search(app: AppHandle, keyword: String, page_num: i64) -> CommandResult<SearchResult> {
    let manhuagui_client = app.get_manhuagui_client();

    let search_result = manhuagui_client
        .search(&keyword, page_num)
        .await
        .map_err(|err| CommandError::from("搜索失败", err))?;
    tracing::debug!("搜索成功");
    Ok(search_result)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_comic(app: AppHandle, id: i64) -> CommandResult<Comic> {
    let comic = utils::get_comic(&app, id)
        .await
        .map_err(|err| CommandError::from(&format!("获取漫画`{id}`的信息失败"), err))?;
    tracing::debug!("获取漫画信息成功");
    Ok(comic)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_favorite(app: AppHandle, page_num: i64) -> CommandResult<GetFavoriteResult> {
    let manhuagui_client = app.get_manhuagui_client();

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
pub fn save_metadata(app: AppHandle, mut comic: Comic) -> CommandResult<()> {
    let config = app.get_config();

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
        .context(format!("创建目录`{}`失败", metadata_dir.display()))
        .map_err(|err| CommandError::from(&format!("`{comic_title}`的元数据保存失败"), err))?;

    std::fs::write(&metadata_path, comic_json)
        .context(format!("写入文件`{}`失败", metadata_path.display()))
        .map_err(|err| CommandError::from(&format!("`{comic_title}`的元数据保存失败"), err))?;

    tracing::debug!("`{comic_title}`的元数据保存成功");
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_downloaded_comics(app: AppHandle) -> CommandResult<Vec<Comic>> {
    let config = app.get_config();

    let download_dir = config.read().download_dir.clone();
    // 遍历下载目录，获取所有元数据文件的路径和修改时间
    let mut metadata_path_with_modify_time = std::fs::read_dir(&download_dir)
        .context(format!("读取下载目录`{}`失败", download_dir.display()))
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
        .filter_map(
            |(metadata_path, _)| match Comic::from_metadata(&app, metadata_path) {
                Ok(comic) => Some(comic),
                Err(err) => {
                    let err_title = format!("读取元数据文件`{}`失败", metadata_path.display());
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    None
                }
            },
        )
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
pub async fn update_downloaded_comics(app: AppHandle) -> CommandResult<()> {
    let config = app.get_config();
    let download_manager = app.get_download_manager();

    // 从下载目录中获取已下载的漫画
    let downloaded_comics = get_downloaded_comics(app.clone())?;
    // 发送正在获取漫画事件
    let total = downloaded_comics.len() as i64;
    let interval_sec = config.read().update_get_comic_interval_sec;
    let _ = UpdateDownloadedComicsEvent::GetComicStart { total }.emit(&app);

    // 获取已下载漫画的最新信息，不用并发是有意为之，防止被封IP
    for (i, downloaded_comic) in downloaded_comics.iter().enumerate() {
        let comic_id = downloaded_comic.id;
        let comic_title = &downloaded_comic.title;
        let current = (i + 1) as i64;
        let _ = UpdateDownloadedComicsEvent::GetComicProgress { current, total }.emit(&app);
        // 获取最新的漫画信息
        let comic = match utils::get_comic(&app, comic_id).await {
            Ok(comic) => comic,
            Err(err) => {
                let err_title = format!("更新库存过程中，获取漫画`{comic_title}`失败，已跳过");
                let err = err.context("可能是频率太高，请手动去`配置`里调整`更新库存时，每处理完一个已下载的漫画后休息`");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                sleep(Duration::from_secs(interval_sec)).await;
                continue;
            }
        };

        let has_downloaded_group = comic.groups.iter().any(|(_, chapter_infos)| {
            chapter_infos
                .iter()
                .any(|chapter_info| chapter_info.is_downloaded.unwrap_or(false))
        });

        if !has_downloaded_group {
            sleep(Duration::from_secs(interval_sec)).await;
            continue;
        }

        let downloaded_groups: HashMap<&String, &Vec<ChapterInfo>> = comic
            .groups
            .iter()
            .filter_map(|(group_name, chapter_infos)| {
                chapter_infos
                    .iter()
                    .any(|chapter_info| chapter_info.is_downloaded.unwrap_or(false))
                    .then_some((group_name, chapter_infos))
            })
            .collect();

        if downloaded_groups.is_empty() {
            sleep(Duration::from_secs(interval_sec)).await;
            continue;
        }

        // 获取downloaded_groups中所有未下载的章节
        let chapter_infos: Vec<&ChapterInfo> = downloaded_groups
            .values()
            .flat_map(|chapter_infos| {
                chapter_infos
                    .iter()
                    .filter(|chapter_info| !chapter_info.is_downloaded.unwrap_or(false))
            })
            .collect();

        if chapter_infos.is_empty() {
            sleep(Duration::from_secs(interval_sec)).await;
            continue;
        }

        let _ = UpdateDownloadedComicsEvent::CreateDownloadTasksStart {
            comic_id,
            comic_title: comic_title.clone(),
            current: 0,
            total: chapter_infos.len() as i64,
        }
        .emit(&app);

        for (i, chapter_info) in chapter_infos.into_iter().enumerate() {
            let chapter_id = chapter_info.chapter_id;
            let current = (i + 1) as i64;

            let _ = download_manager.create_download_task(comic.clone(), chapter_id);

            let _ = UpdateDownloadedComicsEvent::CreateDownloadTaskProgress { comic_id, current }
                .emit(&app);

            sleep(Duration::from_millis(100)).await;
        }

        let _ = UpdateDownloadedComicsEvent::CreateDownloadTasksEnd { comic_id }.emit(&app);

        sleep(Duration::from_secs(interval_sec)).await;
    }

    let _ = UpdateDownloadedComicsEvent::GetComicEnd.emit(&app);

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
        .context(format!("读取日志目录`{}`失败", logs_dir.display()))
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
pub fn create_download_task(app: AppHandle, comic: Comic, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    let comic_title = comic.title.clone();
    download_manager
        .create_download_task(comic, chapter_id)
        .map_err(|err| {
            let err_title = format!("`{comic_title}`的章节ID为`{chapter_id}`的下载任务创建失败");
            CommandError::from(&err_title, err)
        })?;
    tracing::debug!("下载任务创建成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn pause_download_task(app: AppHandle, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    download_manager
        .pause_download_task(chapter_id)
        .map_err(|err| CommandError::from(&format!("暂停章节ID为`{chapter_id}`的下载任务"), err))?;
    tracing::debug!("暂停章节ID为`{chapter_id}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn resume_download_task(app: AppHandle, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

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
pub fn cancel_download_task(app: AppHandle, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    download_manager
        .cancel_download_task(chapter_id)
        .map_err(|err| {
            CommandError::from(&format!("取消章节ID为`{chapter_id}`的下载任务失败"), err)
        })?;
    tracing::debug!("取消章节ID为`{chapter_id}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic(app: AppHandle, mut comic: Comic) -> Comic {
    comic.update_fields(&app);

    comic
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic_in_favorite(app: AppHandle, mut comic: ComicInFavorite) -> ComicInFavorite {
    comic.update_fields(&app);

    comic
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic_in_search(app: AppHandle, mut comic: ComicInSearch) -> ComicInSearch {
    comic.update_fields(&app);

    comic
}
