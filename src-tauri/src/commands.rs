use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::AtomicI64, Arc},
};

use anyhow::{anyhow, Context};
use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;
use tokio::{sync::Semaphore, task::JoinSet};
use walkdir::WalkDir;

use crate::{
    config::Config,
    copy_client::CopyClient,
    download_manager::DownloadManager,
    errors::{CommandError, CommandResult},
    events::UpdateDownloadedComicsEvent,
    export,
    extensions::{AnyhowErrorToStringChain, WalkDirEntryExt},
    logger,
    responses::{
        ChapterInGetChaptersRespData, GetChapterRespData, LoginRespData, UserProfileRespData,
    },
    types::{Comic, ComicInFavorite, ComicInSearch, GetFavoriteResult, SearchResult},
    utils,
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
    app: AppHandle,
    copy_client: State<'_, CopyClient>,
    keyword: String,
    page_num: i64,
) -> CommandResult<SearchResult> {
    let search_resp_data = copy_client
        .search(&keyword, page_num)
        .await
        .map_err(|err| CommandError::from("搜索失败", err))?;

    let search_result = SearchResult::from_resp_data(&app, search_resp_data)
        .map_err(|err| CommandError::from("搜索失败", err))?;

    Ok(search_result)
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
    let comic = Comic::from_resp_data(&app, get_comic_resp_data, groups_chapters)
        .map_err(|err| CommandError::from("获取漫画信息失败", err))?;

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
    app: AppHandle,
    copy_client: State<'_, CopyClient>,
    page_num: i64,
) -> CommandResult<GetFavoriteResult> {
    let get_favorite_resp_data = copy_client
        .get_favorite(page_num)
        .await
        .map_err(|err| CommandError::from("获取收藏夹失败", err))?;

    let get_favorite_result = GetFavoriteResult::from_resp_data(&app, get_favorite_resp_data)
        .map_err(|err| CommandError::from("获取收藏夹失败", err))?;

    Ok(get_favorite_result)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn save_metadata(comic: Comic) -> CommandResult<()> {
    let comic_title = &comic.comic.name;
    comic
        .save_metadata()
        .map_err(|err| CommandError::from(&format!("`{comic_title}`保存元数据失败"), err))?;

    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_downloaded_comics(config: State<RwLock<Config>>) -> Vec<Comic> {
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
            .map_err(anyhow::Error::from)
            .context(format!("获取`{}`的metadata失败", path.display()))
        {
            Ok(metadata) => metadata,
            Err(err) => {
                let err_title = "获取已下载漫画的过程中遇到错误，已跳过";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                continue;
            }
        };

        let modify_time = match metadata
            .modified()
            .map_err(anyhow::Error::from)
            .context(format!("获取`{}`的修改时间失败", path.display()))
        {
            Ok(modify_time) => modify_time,
            Err(err) => {
                let err_title = "获取已下载漫画的过程中遇到错误，已跳过";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                continue;
            }
        };

        metadata_path_and_modify_time_pairs.push((path.to_path_buf(), modify_time));
    }
    // 按照文件修改时间排序，最新的排在最前面
    metadata_path_and_modify_time_pairs.sort_by(|(_, a), (_, b)| b.cmp(a));

    let mut downloaded_comics = Vec::new();
    for (metadata_path, _) in metadata_path_and_modify_time_pairs {
        match Comic::from_metadata(&metadata_path).context(format!(
            "从元数据`{}`转为Comic失败",
            metadata_path.display()
        )) {
            Ok(comic) => downloaded_comics.push(comic),
            Err(err) => {
                let err_title = "获取已下载漫画的过程中遇到错误，已跳过";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
            }
        }
    }

    // 按照漫画ID分组，以方便去重
    let mut comics_by_path_word: IndexMap<String, Vec<Comic>> = IndexMap::new();
    for comic in downloaded_comics {
        comics_by_path_word
            .entry(comic.comic.path_word.clone())
            .or_default()
            .push(comic);
    }

    let mut unique_comics = Vec::new();
    for (_comic_path_word, mut comics) in comics_by_path_word {
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
            let comic_title = &comics[0].comic.name;
            let err_title = "获取已下载漫画的过程中遇到错误";
            let string_chain = anyhow!("所有版本路径: [{dir_paths_string}]")
                .context(format!(
                    "此次获取已下载漫画的结果中只保留版本`{}`",
                    chosen_download_dir.display()
                ))
                .context(format!(
                    "漫画`{comic_title}`在下载目录里有多个版本，请手动处理，只保留一个版本"
                ))
                .to_string_chain();
            tracing::error!(err_title, message = string_chain);
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
pub fn export_cbz(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.comic.name.clone();
    export::cbz(&app, &comic)
        .context(format!("漫画 {comic_title} 导出cbz失败"))
        .map_err(|err| CommandError::from("漫画导出cbz失败", err))?;
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_pdf(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let comic_title = comic.comic.name.clone();
    export::pdf(&app, &comic)
        .context(format!("漫画`{comic_title}`导出pdf失败"))
        .map_err(|err| CommandError::from("漫画导出pdf失败", err))?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn create_download_task(
    download_manager: State<DownloadManager>,
    comic: Comic,
    chapter_uuid: String,
) -> CommandResult<()> {
    let comic_title = comic.comic.name.clone();
    download_manager
        .create_download_task(comic, &chapter_uuid)
        .map_err(|err| {
            let err_title = format!("`{comic_title}`的章节ID为`{chapter_uuid}`的下载任务创建失败");
            CommandError::from(&err_title, err)
        })?;
    tracing::debug!("创建章节ID为`{chapter_uuid}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn pause_download_task(
    download_manager: State<DownloadManager>,
    chapter_uuid: String,
) -> CommandResult<()> {
    download_manager
        .pause_download_task(&chapter_uuid)
        .map_err(|err| {
            CommandError::from(&format!("暂停章节ID为`{chapter_uuid}`的下载任务失败"), err)
        })?;
    tracing::debug!("暂停章节ID为`{chapter_uuid}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn resume_download_task(
    download_manager: State<DownloadManager>,
    chapter_uuid: String,
) -> CommandResult<()> {
    download_manager
        .resume_download_task(&chapter_uuid)
        .map_err(|err| {
            CommandError::from(&format!("恢复章节ID为`{chapter_uuid}`的下载任务失败"), err)
        })?;
    tracing::debug!("恢复章节ID为`{chapter_uuid}`的下载任务成功");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn cancel_download_task(
    download_manager: State<DownloadManager>,
    chapter_uuid: String,
) -> CommandResult<()> {
    download_manager
        .cancel_download_task(&chapter_uuid)
        .map_err(|err| {
            CommandError::from(&format!("取消章节ID为`{chapter_uuid}`的下载任务失败"), err)
        })?;
    tracing::debug!("取消章节ID为`{chapter_uuid}`的下载任务成功");
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
    let downloaded_comics = get_downloaded_comics(app.state::<RwLock<Config>>());
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
            let comic_title = &comic.comic.name;
            comic.save_metadata().map_err(|err| {
                CommandError::from(&format!("`{comic_title}`保存元数据失败"), err)
            })?;

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

    let mut downloaded_comic_and_groups_pairs = Vec::new();
    for comic in latest_comics {
        // 先过滤出至少有一个已下载章节的组
        let downloaded_groups = comic
            .comic
            .groups
            .iter()
            .filter_map(|(group_path_word, chapter_infos)| {
                chapter_infos
                    .iter()
                    .any(|chapter_info| chapter_info.is_downloaded.unwrap_or(false))
                    .then_some((group_path_word.clone(), chapter_infos.clone()))
            })
            .collect::<HashMap<_, _>>();

        if !downloaded_groups.is_empty() {
            downloaded_comic_and_groups_pairs.push((comic, downloaded_groups));
        }
    }
    // 给需要下载的章节创建下载任务
    for (comic, downloaded_groups) in downloaded_comic_and_groups_pairs {
        // 过滤出未下载的章节
        let chapters_to_download: Vec<_> = downloaded_groups
            .into_iter()
            .flat_map(|(_, chapter_infos)| chapter_infos)
            .filter(|chapter_info| !chapter_info.is_downloaded.unwrap_or(false))
            .collect();

        for chapter_info in chapters_to_download {
            let comic = comic.clone();
            let chapter_uuid = &chapter_info.chapter_uuid;
            download_manager
                .create_download_task(comic, chapter_uuid)
                .map_err(|err| {
                    CommandError::from(&format!("创建章节ID为`{chapter_uuid}`的下载任务失败"), err)
                })?;
        }
    }

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

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn show_path_in_file_manager(app: AppHandle, path: &str) -> CommandResult<()> {
    app.opener()
        .reveal_item_in_dir(path)
        .context(format!("在文件管理器中打开`{path}`失败"))
        .map_err(|err| CommandError::from("在文件管理器中打开失败", err))?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic(app: AppHandle, mut comic: Comic) -> CommandResult<Comic> {
    let comic_title = comic.comic.name.clone();

    let path_word_to_dir_map = utils::create_path_word_to_dir_map(&app)
        .context("创建漫画路径词到下载目录映射失败")
        .map_err(|err| {
            CommandError::from(&format!("漫画`{comic_title}`同步Comic的字段失败"), err)
        })?;

    comic.update_fields(&path_word_to_dir_map).map_err(|err| {
        CommandError::from(&format!("漫画`{comic_title}`同步Comic的字段失败"), err)
    })?;

    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic_in_favorite(
    app: AppHandle,
    mut comic: ComicInFavorite,
) -> CommandResult<ComicInFavorite> {
    let comic_title = comic.name.clone();

    let path_word_to_dir_map = utils::create_path_word_to_dir_map(&app)
        .context("创建漫画路径词到下载目录映射失败")
        .map_err(|err| {
            let err_title = format!("漫画`{comic_title}`同步ComicInFavorite的字段失败");
            CommandError::from(&err_title, err)
        })?;

    comic.update_fields(&path_word_to_dir_map);

    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic_in_search(
    app: AppHandle,
    mut comic: ComicInSearch,
) -> CommandResult<ComicInSearch> {
    let comic_title = comic.name.clone();

    let path_word_to_dir_map = utils::create_path_word_to_dir_map(&app)
        .context("创建漫画路径词到下载目录映射失败")
        .map_err(|err| {
            let err_title = format!("漫画`{comic_title}`同步ComicInSearch的字段失败");
            CommandError::from(&err_title, err)
        })?;

    comic.update_fields(&path_word_to_dir_map);

    Ok(comic)
}
