use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::AtomicI64, Arc},
};

use anyhow::{anyhow, Context};
use parking_lot::{Mutex, RwLock};
use path_slash::PathBufExt;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use tokio::{sync::Semaphore, task::JoinSet};

use crate::{
    config::Config,
    copy_client::CopyClient,
    download_manager::DownloadManager,
    errors::{CommandError, CommandResult},
    events::UpdateDownloadedComicsEvent,
    export, logger,
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
pub async fn register(
    copy_client: State<'_, CopyClient>,
    username: String,
    password: String,
) -> CommandResult<()> {
    copy_client
        .register(&username, &password)
        .await
        .map_err(|err| CommandError::from("注册失败", err))?;
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn login(
    copy_client: State<'_, CopyClient>,
    username: String,
    password: String,
) -> CommandResult<LoginRespData> {
    let login_resp_data = copy_client
        .login(&username, &password)
        .await
        .map_err(|err| CommandError::from("登录失败", err))?;
    Ok(login_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_user_profile(
    copy_client: State<'_, CopyClient>,
) -> CommandResult<UserProfileRespData> {
    let user_profile_resp_data = copy_client
        .get_user_profile()
        .await
        .map_err(|err| CommandError::from("获取用户信息失败", err))?;
    Ok(user_profile_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn search(
    copy_client: State<'_, CopyClient>,
    keyword: String,
    page_num: i64,
) -> CommandResult<SearchRespData> {
    let search_resp_data = copy_client
        .search(&keyword, page_num)
        .await
        .map_err(|err| CommandError::from("搜索失败", err))?;
    Ok(search_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_comic(
    app: AppHandle,
    copy_client: State<'_, CopyClient>,
    comic_path_word: &str,
) -> CommandResult<Comic> {
    let get_comic_resp_data = copy_client
        .get_comic(comic_path_word)
        .await
        .map_err(|err| CommandError::from("获取漫画信息失败", err))?;
    // TODO: 这里可以并发获取groups_chapters
    let mut groups_chapters = HashMap::new();
    for group_path_word in get_comic_resp_data.groups.keys() {
        let chapters = copy_client
            .get_group_chapters(comic_path_word, group_path_word)
            .await
            .map_err(|err| CommandError::from("获取漫画信息失败", err))?;
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
        .await
        .map_err(|err| CommandError::from("获取分组章节失败", err))?;
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
        .await
        .map_err(|err| CommandError::from("获取章节信息失败", err))?;
    Ok(get_chapter_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_favorite(
    copy_client: State<'_, CopyClient>,
    page_num: i64,
) -> CommandResult<GetFavoriteRespData> {
    let get_favorite_resp_data = copy_client
        .get_favorite(page_num)
        .await
        .map_err(|err| CommandError::from("获取收藏夹失败", err))?;
    Ok(get_favorite_resp_data)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn download_chapters(
    download_manager: State<'_, DownloadManager>,
    chapters: Vec<ChapterInfo>,
) -> CommandResult<()> {
    for ep in chapters {
        download_manager
            .submit_chapter(ep)
            .await
            .map_err(|err| CommandError::from("创建章节下载任务失败", err))?;
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
    let comic_json = serde_json::to_string_pretty(&comic)
        .context(format!(
            "{comic_title} 的元数据保存失败，将Comic序列化为json失败"
        ))
        .map_err(|err| CommandError::from("保存漫画元数据失败", err))?;

    let download_dir = config.read().download_dir.clone();
    let metadata_dir = download_dir.join(&comic_title);
    let metadata_path = metadata_dir.join("元数据.json");

    std::fs::create_dir_all(&metadata_dir)
        .context(format!(
            "{comic_title} 的元数据保存失败，创建目录 {metadata_dir:?} 失败"
        ))
        .map_err(|err| CommandError::from("保存漫画元数据失败", err))?;

    std::fs::write(&metadata_path, comic_json)
        .context(format!(
            "{comic_title} 的元数据保存失败，写入文件 {metadata_path:?} 失败"
        ))
        .map_err(|err| CommandError::from("保存漫画元数据失败", err))?;

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
        ))
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
        // TODO: 如果读取元数据失败，应该发送错误Event通知前端，然后才跳过
        .filter_map(|(metadata_path, _)| Comic::from_metadata(&app, metadata_path).ok())
        .collect::<Vec<_>>();

    Ok(downloaded_comics)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_cbz(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.comic.name.clone();
    export::cbz(&app, comic)
        .context(format!("漫画 {comic_title} 导出cbz失败"))
        .map_err(|err| CommandError::from("漫画导出cbz失败", err))?;
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_pdf(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.comic.name.clone();
    export::pdf(&app, comic)
        .context(format!("漫画`{comic_title}`导出pdf失败"))
        .map_err(|err| CommandError::from("漫画导出pdf失败", err))?;
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
    let latest_comics = Arc::new(Mutex::new(Vec::new()));
    // 限制并发数为10
    let sem = Arc::new(Semaphore::new(10));
    let current = Arc::new(AtomicI64::new(0));
    // 发送正在获取漫画事件
    let total = downloaded_comics.len() as i64;
    let _ = UpdateDownloadedComicsEvent::GettingComics { total }.emit(&app);
    let mut join_set = JoinSet::new();
    // 获取已下载漫画的最新信息
    for downloaded_comic in downloaded_comics {
        let sem = sem.clone();
        let latest_comics = latest_comics.clone();
        let current = current.clone();
        let app = app.clone();
        join_set.spawn(async move {
            // 获取最新的漫画信息
            let permit = sem
                .acquire()
                .await
                .map_err(|err| CommandError::from("获取漫画信息失败", anyhow::Error::from(err)))?;
            let client = app.state::<CopyClient>();
            let path_word = &downloaded_comic.comic.path_word;
            let comic = match get_comic(app.clone(), client, path_word).await {
                Ok(comic) => comic,
                Err(err) => {
                    // 发送获取漫画失败事件
                    let _ = UpdateDownloadedComicsEvent::GetComicError {
                        comic_title: downloaded_comic.comic.name.clone(),
                        err_msg: err.err_message,
                    }
                    .emit(&app);
                    let current = current.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    // 发送获取到漫画事件
                    let _ = UpdateDownloadedComicsEvent::ComicGot { current, total }.emit(&app);
                    return Ok::<(), CommandError>(());
                }
            };
            drop(permit);
            // 将最新的漫画信息保存到元数据文件
            save_metadata(app.state::<RwLock<Config>>(), comic.clone())?;

            latest_comics.lock().push(comic);
            let current = current.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            // 发送获取到漫画事件
            let _ = UpdateDownloadedComicsEvent::ComicGot { current, total }.emit(&app);
            Ok::<(), CommandError>(())
        });
    }
    // 等待所有请求完成
    while let Some(Ok(get_comic_result)) = join_set.join_next().await {
        // 如果有请求失败，直接返回错误
        get_comic_result?;
    }
    // 至此，已下载的漫画的最新信息已获取完毕
    let latest_comics = std::mem::take(&mut *latest_comics.lock());
    let chapters_to_download = latest_comics
        .into_iter()
        .filter_map(|comic| {
            // 先过滤出每个漫画中至少有一个已下载章节的组
            let downloaded_group = comic
                .comic
                .groups
                .into_iter()
                .filter_map(|(group_path_word, chapter_infos)| {
                    // 检查当前组是否有已下载章节，如果有，则返回组路径和章节信息，否则返回None(跳过)
                    chapter_infos
                        .iter()
                        .any(|chapter_info| chapter_info.is_downloaded.unwrap_or(false))
                        .then_some((group_path_word, chapter_infos))
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
    download_chapters(download_manager, chapters_to_download).await?;
    // 发送下载任务创建完成事件
    let _ = UpdateDownloadedComicsEvent::DownloadTaskCreated.emit(&app);

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

#[tauri::command(async)]
#[specta::specta]
pub fn show_path_in_file_manager(path: &str) -> CommandResult<()> {
    let path = PathBuf::from_slash(path);
    if !path.exists() {
        let err_title = format!("在文件管理器中打开`{path:?}`失败");
        return Err(CommandError::from(
            &err_title,
            anyhow!("路径`{path:?}`不存在"),
        ));
    }
    showfile::show_path_in_file_manager(path);
    Ok(())
}
