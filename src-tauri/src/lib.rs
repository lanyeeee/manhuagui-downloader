mod commands;
mod config;
mod decrypt;
mod download_manager;
mod errors;
mod events;
mod extensions;
mod manhuagui_client;
mod types;
mod utils;

use anyhow::Context;
use config::Config;
use download_manager::DownloadManager;
use events::DownloadEvent;
use manhuagui_client::ManhuaguiClient;
use parking_lot::RwLock;
use tauri::{Manager, Wry};

use crate::commands::*;

fn generate_context() -> tauri::Context<Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri_specta::Builder::<Wry>::new()
        .commands(tauri_specta::collect_commands![
            greet,
            get_config,
            save_config,
            login,
            get_user_profile,
            search,
            get_comic,
            download_chapters,
        ])
        .events(tauri_specta::collect_events![DownloadEvent]);

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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_data_dir = app
                .path()
                .app_data_dir()
                .context("failed to get app data dir")?;

            std::fs::create_dir_all(&app_data_dir)
                .context(format!("failed to create app data dir: {app_data_dir:?}"))?;

            let config = RwLock::new(Config::new(app.handle())?);
            app.manage(config);

            let manhuagui_client = ManhuaguiClient::new(app.handle().clone());
            app.manage(manhuagui_client);

            let download_manager = DownloadManager::new(app.handle());
            app.manage(download_manager);

            Ok(())
        })
        .run(generate_context())
        .expect("error while running tauri application");
}
