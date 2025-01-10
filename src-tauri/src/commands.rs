use anyhow::Context;
use parking_lot::RwLock;
use tauri::{AppHandle, State};

use crate::{
    config::Config, errors::CommandResult, manhuagui_client::ManhuaguiClient, types::UserProfile,
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
