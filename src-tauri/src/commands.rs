use parking_lot::RwLock;
use tauri::{AppHandle, State};

use crate::{
    config::Config,
    copy_client::CopyClient,
    errors::CommandResult,
    responses::{
        ChapterDetailRespData, ComicRespData, LoginRespData, SearchRespData, UserProfileRespData,
    },
};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command(async)]
#[specta::specta]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_config(config: State<RwLock<Config>>) -> Config {
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
    copy_client: State<'_, CopyClient>,
    username: String,
    password: String,
) -> CommandResult<LoginRespData> {
    let login_resp_data = copy_client.login(&username, &password).await?;
    Ok(login_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_user_profile(
    copy_client: State<'_, CopyClient>,
) -> CommandResult<UserProfileRespData> {
    let user_profile_resp_data = copy_client.get_user_profile().await?;
    Ok(user_profile_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn search(
    copy_client: State<'_, CopyClient>,
    keyword: String,
    page_num: i64,
) -> CommandResult<SearchRespData> {
    let search_resp_data = copy_client.search(&keyword, page_num).await?;
    Ok(search_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_comic(
    copy_client: State<'_, CopyClient>,
    path_word: String,
) -> CommandResult<ComicRespData> {
    let comic_resp_data = copy_client.get_comic(path_word).await?;
    Ok(comic_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_chapters(
    copy_client: State<'_, CopyClient>,
    comic_path_word: &str,
    group_path_word: &str,
) -> CommandResult<Vec<ChapterDetailRespData>> {
    let chapters = copy_client
        .get_chapters(comic_path_word, group_path_word)
        .await?;
    Ok(chapters)
}
