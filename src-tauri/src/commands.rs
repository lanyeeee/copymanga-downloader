use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use path_slash::PathBufExt;
use tauri::{AppHandle, State};

use crate::{
    config::Config,
    copy_client::CopyClient,
    download_manager::DownloadManager,
    errors::CommandResult,
    responses::{
        ChapterInGetChaptersRespData, GetChapterRespData, GetFavoriteRespData, LoginRespData,
        SearchRespData, UserProfileRespData,
    },
    types::{ChapterInfo, Comic},
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
pub async fn register(
    copy_client: State<'_, CopyClient>,
    username: String,
    password: String,
) -> CommandResult<()> {
    copy_client.register(&username, &password).await?;
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
    app: AppHandle,
    copy_client: State<'_, CopyClient>,
    comic_path_word: &str,
) -> CommandResult<Comic> {
    let get_comic_resp_data = copy_client.get_comic(comic_path_word).await?;
    // TODO: 这里可以并发获取groups_chapters
    let mut groups_chapters = HashMap::new();
    for group_path_word in get_comic_resp_data.groups.keys() {
        let chapters = copy_client
            .get_group_chapters(comic_path_word, group_path_word)
            .await?;
        groups_chapters.insert(group_path_word.clone(), chapters);
    }
    let comic = Comic::from_resp_data(&app, get_comic_resp_data, groups_chapters);

    Ok(comic)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_group_chapters(
    copy_client: State<'_, CopyClient>,
    comic_path_word: &str,
    group_path_word: &str,
) -> CommandResult<Vec<ChapterInGetChaptersRespData>> {
    let chapters = copy_client
        .get_group_chapters(comic_path_word, group_path_word)
        .await?;
    Ok(chapters)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_chapter(
    copy_client: State<'_, CopyClient>,
    comic_path_word: &str,
    chapter_uuid: &str,
) -> CommandResult<GetChapterRespData> {
    let get_chapter_resp_data = copy_client
        .get_chapter(comic_path_word, chapter_uuid)
        .await?;
    Ok(get_chapter_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_favorite(
    copy_client: State<'_, CopyClient>,
    page_num: i64,
) -> CommandResult<GetFavoriteRespData> {
    let get_favorite_resp_data = copy_client.get_favorite(page_num).await?;
    Ok(get_favorite_resp_data)
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
#[allow(clippy::needless_pass_by_value)]
pub fn save_metadata(config: State<RwLock<Config>>, mut comic: Comic) -> CommandResult<()> {
    // 将所有章节的is_downloaded字段设置为None，这样能使is_downloaded字段在序列化时被忽略
    for chapter_infos in comic.comic.groups.values_mut() {
        for chapter_info in chapter_infos.iter_mut() {
            chapter_info.is_downloaded = None;
        }
    }

    let comic_title = comic.comic.name.clone();
    let comic_json = serde_json::to_string_pretty(&comic).context(format!(
        "{comic_title} 的元数据保存失败，将Comic序列化为json失败"
    ))?;

    let download_dir = config.read().download_dir.clone();
    let metadata_dir = download_dir.join(&comic_title);
    let metadata_path = metadata_dir.join("元数据.json");

    std::fs::create_dir_all(&metadata_dir).context(format!(
        "{comic_title} 的元数据保存失败，创建目录 {metadata_dir:?} 失败"
    ))?;

    std::fs::write(&metadata_path, comic_json).context(format!(
        "{comic_title} 的元数据保存失败，写入文件 {metadata_path:?} 失败"
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
    // 遍历下载目录，获取所有已下载的漫画
    let downloaded_comics = std::fs::read_dir(&download_dir)
        .context(format!("读取下载目录 {download_dir:?} 失败"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata_path = entry.path().join("元数据.json");
            if !metadata_path.exists() {
                return None;
            }
            // TODO: 如果读取元数据失败，应该发送错误Event通知前端，然后才跳过
            let comic = Comic::from_metadata(&app, &metadata_path).ok()?;
            Some(comic)
        })
        .collect::<Vec<_>>();

    Ok(downloaded_comics)
}

#[tauri::command(async)]
#[specta::specta]
pub fn show_path_in_file_manager(path: &str) -> CommandResult<()> {
    let path = PathBuf::from_slash(path);
    if !path.exists() {
        return Err(anyhow!("路径`{path:?}`不存在").into());
    }
    showfile::show_path_in_file_manager(path);
    Ok(())
}
