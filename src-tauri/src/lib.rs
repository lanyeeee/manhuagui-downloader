mod commands;
mod config;
mod decrypt;
mod downloader;
mod errors;
mod events;
mod export;
mod extensions;
mod logger;
mod manhuagui_client;
mod types;
mod utils;

use config::Config;
use eyre::WrapErr;
use manhuagui_client::ManhuaguiClient;
use parking_lot::RwLock;
use tauri::{Manager, Wry};

use crate::{
    commands::*,
    downloader::download_manager::DownloadManager,
    errors::install_custom_eyre_handler,
    events::{
        DownloadEvent, ExportCbzEvent, ExportPdfEvent, LogEvent, UpdateDownloadedComicsEvent,
    },
};

fn generate_context() -> tauri::Context<Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_custom_eyre_handler().unwrap();

    let builder = tauri_specta::Builder::<Wry>::new()
        .commands(tauri_specta::collect_commands![
            greet,
            get_config,
            save_config,
            login,
            get_user_profile,
            search,
            get_comic,
            get_favorite,
            get_downloaded_comics,
            export_cbz,
            export_pdf,
            update_downloaded_comics,
            get_logs_dir_size,
            show_path_in_file_manager,
            create_download_task,
            pause_download_task,
            resume_download_task,
            delete_download_task,
            get_synced_comic,
            get_synced_comic_in_favorite,
            get_synced_comic_in_search,
            open_log_file,
        ])
        .events(tauri_specta::collect_events![
            DownloadEvent,
            ExportCbzEvent,
            ExportPdfEvent,
            UpdateDownloadedComicsEvent,
            LogEvent,
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .formatter(specta_typescript::formatter::prettier)
                .header("// @ts-nocheck"), // 跳过检查
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_data_dir = app
                .path()
                .app_data_dir()
                .wrap_err("获取app_data_dir目录失败")?;

            std::fs::create_dir_all(&app_data_dir).wrap_err(format!(
                "创建app_data_dir目录`{}`失败",
                app_data_dir.display()
            ))?;

            let config = RwLock::new(Config::new(app.handle()).wrap_err("创建Config失败")?);
            app.manage(config);

            let manhuagui_client = ManhuaguiClient::new(app.handle().clone());
            app.manage(manhuagui_client);

            let download_manager = DownloadManager::new(app.handle());
            app.manage(download_manager);

            logger::init(app.handle()).wrap_err("初始化日志系统失败")?;

            Ok(())
        })
        .run(generate_context())
        .expect("error while running tauri application");
}
