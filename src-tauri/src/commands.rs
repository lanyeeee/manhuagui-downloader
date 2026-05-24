use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    time::Duration,
};

use eyre::{eyre, WrapErr};
use indexmap::IndexMap;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;
use tokio::time::sleep;
use tracing::instrument;
use walkdir::WalkDir;

use crate::{
    config::Config,
    errors::{CommandError, CommandResult},
    events::UpdateDownloadedComicsEvent,
    export,
    extensions::{AppHandleExt, EyreReportToMessage, WalkDirEntryExt},
    logger,
    types::{
        ChapterInfo, Comic, ComicInFavorite, ComicInSearch, GetFavoriteResult, LogMetadata,
        SearchResult, UserProfile,
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
#[instrument(level = "error", skip_all)]
pub fn get_config(app: AppHandle) -> Config {
    let config = app.get_config().read().clone();

    tracing::debug!("获取配置成功");
    config
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
#[instrument(level = "error", skip_all)]
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
#[instrument(level = "error", skip_all)]
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
#[instrument(level = "error", skip_all)]
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
#[instrument(
    level = "error",
    skip_all,
    fields(keyword = keyword, page_num = page_num)
)]
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
#[instrument(level = "error", skip_all, fields(comic_id = id))]
pub async fn get_comic(app: AppHandle, id: i64) -> CommandResult<Comic> {
    let comic = utils::get_comic(&app, id)
        .await
        .map_err(|err| CommandError::from("获取漫画信息失败", err))?;
    tracing::debug!("获取漫画信息成功");
    Ok(comic)
}

#[tauri::command(async)]
#[specta::specta]
#[instrument(level = "error", skip_all, fields(page_num = page_num))]
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
#[instrument(level = "error", skip_all)]
pub fn get_downloaded_comics(app: AppHandle) -> Vec<Comic> {
    let config = app.get_config();

    let download_dir = config.read().download_dir.clone();
    // 遍历下载目录，获取所有元数据文件的路径和修改时间
    let mut metadata_path_and_modify_time_pairs = Vec::new();
    for entry in WalkDir::new(&download_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if !entry.is_comic_metadata() {
            continue;
        }

        let metadata = match path
            .metadata()
            .map_err(eyre::Report::from)
            .wrap_err(format!("获取`{}`的metadata失败", path.display()))
        {
            Ok(metadata) => metadata,
            Err(err) => {
                let err_title = "获取已下载漫画的过程中遇到错误，已跳过";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            }
        };

        let modify_time = match metadata
            .modified()
            .map_err(eyre::Report::from)
            .wrap_err(format!("获取`{}`的修改时间失败", path.display()))
        {
            Ok(modify_time) => modify_time,
            Err(err) => {
                let err_title = "获取已下载漫画的过程中遇到错误，已跳过";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            }
        };

        metadata_path_and_modify_time_pairs.push((path.to_path_buf(), modify_time));
    }
    // 按照文件修改时间排序，最新的排在最前面
    metadata_path_and_modify_time_pairs.sort_by(|(_, a), (_, b)| b.cmp(a));

    let mut downloaded_comics = Vec::new();
    for (metadata_path, _) in metadata_path_and_modify_time_pairs {
        match Comic::from_metadata(&metadata_path) {
            Ok(comic) => downloaded_comics.push(comic),
            Err(err) => {
                let err_title = "获取已下载漫画的过程中遇到错误，已跳过";
                let message = err.to_message();
                tracing::error!(err_title, message);
            }
        }
    }

    // 按照漫画ID分组，以方便去重
    let mut comics_by_id: IndexMap<i64, Vec<Comic>> = IndexMap::new();
    for comic in downloaded_comics {
        comics_by_id.entry(comic.id).or_default().push(comic);
    }

    let mut unique_comics = Vec::new();
    for (_comic_id, mut comics) in comics_by_id {
        // 该漫画ID对应的所有漫画下载目录，可能有多个版本，所以需要去重
        let comic_download_dirs: Vec<&PathBuf> = comics
            .iter()
            .filter_map(|comic| comic.comic_download_dir.as_ref())
            .collect();

        if comic_download_dirs.is_empty() {
            // 其实这种情况不应该发生，因为漫画元数据文件应该总是有下载目录的
            continue;
        }

        // 选第一个作为保留的漫画
        let chosen_download_dir = comic_download_dirs[0];

        if comics.len() > 1 {
            let dir_paths_string = comic_download_dirs
                .iter()
                .map(|path| format!("`{}`", path.display()))
                .collect::<Vec<String>>()
                .join(", ");
            // 如果有重复的漫画，打印错误信息
            let comic_title = &comics[0].title;
            let err_title = "获取已下载漫画的过程中遇到错误";
            let message = eyre!("所有版本路径: [{dir_paths_string}]")
                .wrap_err(format!(
                    "此次获取已下载漫画的结果中只保留版本`{}`",
                    chosen_download_dir.display()
                ))
                .wrap_err(format!(
                    "漫画`{comic_title}`在下载目录里有多个版本，请手动处理，只保留一个版本"
                ))
                .to_message();
            tracing::error!(err_title, message);
        }
        // 取第一个作为保留的漫画
        let chosen_comic = comics.remove(0);
        unique_comics.push(chosen_comic);
    }

    unique_comics
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn export_cbz(app: AppHandle, comic: Comic) -> CommandResult<()> {
    export::cbz(&app, &comic).map_err(|err| CommandError::from("漫画导出cbz失败", err))?;
    tracing::debug!("漫画导出cbz成功");
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn export_pdf(app: AppHandle, comic: Comic) -> CommandResult<()> {
    export::pdf(&app, &comic).map_err(|err| CommandError::from("漫画导出pdf失败", err))?;
    tracing::debug!("漫画导出pdf成功");
    Ok(())
}

#[allow(clippy::cast_possible_wrap)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(level = "error", skip_all)]
pub async fn update_downloaded_comics(app: AppHandle) -> CommandResult<()> {
    let config = app.get_config();
    let download_manager = app.get_download_manager();

    // 从下载目录中获取已下载的漫画
    let downloaded_comics = get_downloaded_comics(app.clone());
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
                let err = err.wrap_err(
                    "可能是频率太高，请手动去`配置`里调整`更新库存时，每处理完一个已下载的漫画后休息`",
                );
                let message = err.to_message();
                tracing::error!(err_title, message);
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
#[instrument(level = "error", skip_all)]
pub fn get_logs_dir_size(app: AppHandle) -> CommandResult<u64> {
    let logs_dir = logger::logs_dir(&app)
        .wrap_err("获取日志目录失败")
        .map_err(|err| CommandError::from("获取日志目录大小失败", err))?;
    let logs_dir_size = std::fs::read_dir(&logs_dir)
        .wrap_err(format!("读取日志目录`{}`失败", logs_dir.display()))
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
#[instrument(level = "error", skip_all, fields(path = path))]
pub fn show_path_in_file_manager(app: AppHandle, path: &str) -> CommandResult<()> {
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|err| CommandError::from("在文件管理器中打开失败", err))?;
    tracing::debug!("在文件管理器中打开成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(
    level = "error",
    skip_all,
    fields(
        comic_id = comic.id,
        comic_title = comic.title,
        chapter_id = chapter_id
    )
)]
pub fn create_download_task(app: AppHandle, comic: Comic, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    download_manager
        .create_download_task(comic, chapter_id)
        .map_err(|err| CommandError::from("创建下载任务失败", err))?;
    tracing::debug!("下载任务创建成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
pub fn pause_download_task(app: AppHandle, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    download_manager
        .pause_download_task(chapter_id)
        .map_err(|err| CommandError::from("暂停下载任务失败", err))?;
    tracing::debug!("暂停下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
pub fn resume_download_task(app: AppHandle, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    download_manager
        .resume_download_task(chapter_id)
        .map_err(|err| CommandError::from("恢复下载任务失败", err))?;
    tracing::debug!("恢复下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(level = "error", skip_all, fields(chapter_id = chapter_id))]
pub fn delete_download_task(app: AppHandle, chapter_id: i64) -> CommandResult<()> {
    let download_manager = app.get_download_manager();

    download_manager
        .delete_download_task(chapter_id)
        .map_err(|err| CommandError::from("删除下载任务失败", err))?;
    tracing::debug!("删除下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn get_synced_comic(app: AppHandle, mut comic: Comic) -> CommandResult<Comic> {
    let id_to_dir_map = utils::create_id_to_dir_map(&app)
        .map_err(|err| CommandError::from("同步Comic的字段失败", err))?;

    comic
        .update_fields(&id_to_dir_map)
        .map_err(|err| CommandError::from("同步Comic的字段失败", err))?;

    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn get_synced_comic_in_favorite(
    app: AppHandle,
    mut comic: ComicInFavorite,
) -> CommandResult<ComicInFavorite> {
    let id_to_dir_map = utils::create_id_to_dir_map(&app)
        .map_err(|err| CommandError::from("同步ComicInFavorite的字段失败", err))?;

    comic.update_fields(&id_to_dir_map);

    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(
    level = "error",
    skip_all,
    fields(comic_id = comic.id, comic_title = comic.title)
)]
pub fn get_synced_comic_in_search(
    app: AppHandle,
    mut comic: ComicInSearch,
) -> CommandResult<ComicInSearch> {
    let id_to_dir_map = utils::create_id_to_dir_map(&app)
        .map_err(|err| CommandError::from("同步ComicInSearch的字段失败", err))?;

    comic.update_fields(&id_to_dir_map);

    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
#[instrument(level = "error", skip_all, fields(path = path))]
pub fn open_log_file(path: &str) -> CommandResult<Vec<LogMetadata>> {
    let log_file = File::open(path).map_err(|err| CommandError::from("打开日志文件失败", err))?;
    let reader = BufReader::new(log_file);

    let mut logs = Vec::new();
    let mut line_num = 0;

    for line_result in reader.lines() {
        line_num += 1;

        let line = line_result
            .wrap_err(format!("读取日志文件的第`{line_num}`行失败"))
            .map_err(|err| CommandError::from("打开日志文件失败", err))?;

        if line.trim().is_empty() {
            continue;
        }

        let log: LogMetadata = serde_json::from_str(&line)
            .wrap_err(format!("将日志文件的第`{line_num}`行解析为LogMetadata失败"))
            .map_err(|err| CommandError::from("打开日志文件失败", err))?;

        logs.push(log);
    }

    Ok(logs)
}
