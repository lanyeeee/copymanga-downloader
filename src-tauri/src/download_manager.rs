use std::{
    collections::HashMap,
    io::Cursor,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use bytes::Bytes;
use image::ImageFormat;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::{
    sync::{watch, Semaphore, SemaphorePermit},
    task::JoinSet,
};

use crate::{
    config::Config,
    copy_client::CopyClient,
    errors::{CopyMangaError, RiskControlError},
    events::{DownloadControlRiskEvent, DownloadSpeedEvent, DownloadTaskEvent},
    extensions::AnyhowErrorToStringChain,
    responses::GetChapterRespData,
    types::{ChapterInfo, Comic},
};

/// 用于管理下载任务
///
/// 克隆 `DownloadManager` 的开销极小，性能开销几乎可以忽略不计。
/// 可以放心地在多个线程中传递和使用它的克隆副本。
///
/// 具体来说：
/// - `app` 是 `AppHandle` 类型，根据 `Tauri` 文档，它的克隆开销是极小的。
/// - 其他字段都被 `Arc` 包裹，这些字段的克隆操作仅仅是增加引用计数。
#[derive(Clone)]
pub struct DownloadManager {
    app: AppHandle,
    chapter_sem: Arc<Semaphore>,
    img_sem: Arc<Semaphore>,
    byte_per_sec: Arc<AtomicU64>,
    download_tasks: Arc<RwLock<HashMap<String, DownloadTask>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DownloadTaskState {
    Pending,
    Downloading,
    Paused,
    Cancelled,
    Completed,
    Failed,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let manager = DownloadManager {
            app: app.clone(),
            chapter_sem: Arc::new(Semaphore::new(3)),
            img_sem: Arc::new(Semaphore::new(30)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: Arc::new(RwLock::new(HashMap::new())),
        };

        tauri::async_runtime::spawn(manager.clone().emit_download_speed_loop());

        manager
    }

    #[allow(clippy::cast_precision_loss)]
    async fn emit_download_speed_loop(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let byte_per_sec = self.byte_per_sec.swap(0, Ordering::Relaxed);
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");
            // 发送总进度条下载速度事件
            let _ = DownloadSpeedEvent { speed }.emit(&self.app);
        }
    }

    pub fn create_download_task(&self, comic: Comic, chapter_uuid: &str) -> anyhow::Result<()> {
        use DownloadTaskState::{Downloading, Paused, Pending};
        let mut tasks = self.download_tasks.write();
        if let Some(task) = tasks.get(chapter_uuid) {
            // 如果任务已经存在，且状态是`Pending`、`Downloading`或`Paused`，则不创建新任务
            let state = *task.state_sender.borrow();
            if matches!(state, Pending | Downloading | Paused) {
                return Err(anyhow!("章节ID为`{chapter_uuid}`的下载任务已存在"));
            }
        }
        tasks.remove(chapter_uuid);
        let task = DownloadTask::new(self.app.clone(), comic, chapter_uuid)
            .context("DownloadTask创建失败")?;
        tauri::async_runtime::spawn(task.clone().process());
        tasks.insert(chapter_uuid.to_string(), task);
        Ok(())
    }

    pub fn pause_download_task(&self, chapter_uuid: &str) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(chapter_uuid) else {
            return Err(anyhow!("未找到章节ID为`{chapter_uuid}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Paused);
        Ok(())
    }

    pub fn resume_download_task(&self, chapter_uuid: &str) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(chapter_uuid) else {
            return Err(anyhow!("未找到章节ID为`{chapter_uuid}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Pending);
        Ok(())
    }

    pub fn cancel_download_task(&self, chapter_uuid: &str) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(chapter_uuid) else {
            return Err(anyhow!("未找到章节ID为`{chapter_uuid}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Cancelled);
        Ok(())
    }
}

#[derive(Clone)]
struct DownloadTask {
    app: AppHandle,
    download_manager: DownloadManager,
    comic: Arc<Comic>,
    chapter_info: Arc<ChapterInfo>,
    state_sender: watch::Sender<DownloadTaskState>,
    downloaded_img_count: Arc<AtomicU32>,
    total_img_count: Arc<AtomicU32>,
}

impl DownloadTask {
    fn new(app: AppHandle, comic: Comic, chapter_uuid: &str) -> anyhow::Result<Self> {
        let chapter_info = comic
            .comic
            .groups
            .iter()
            .flat_map(|(_, chapter_infos)| chapter_infos.iter())
            .find(|chapter_info| chapter_info.chapter_uuid == chapter_uuid)
            .cloned()
            .context(format!("未找到章节ID为`{chapter_uuid}`的章节信息"))?;

        let download_manager = app.state::<DownloadManager>().inner().clone();
        let (state_sender, _) = watch::channel(DownloadTaskState::Pending);

        let task = Self {
            app,
            download_manager,
            comic: Arc::new(comic),
            chapter_info: Arc::new(chapter_info),
            state_sender,
            downloaded_img_count: Arc::new(AtomicU32::new(0)),
            total_img_count: Arc::new(AtomicU32::new(0)),
        };

        Ok(task)
    }

    async fn process(self) {
        self.emit_download_task_create_event();

        let download_chapter_task = self.download_chapter();
        tokio::pin!(download_chapter_task);

        let mut state_receiver = self.state_sender.subscribe();
        state_receiver.mark_changed();
        let mut permit = None;
        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            let state_is_pending = *state_receiver.borrow() == DownloadTaskState::Pending;
            tokio::select! {
                () = &mut download_chapter_task, if state_is_downloading && permit.is_some() => break,
                control_flow = self.acquire_chapter_permit(&mut permit), if state_is_pending => {
                    match control_flow {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                },
                _ = state_receiver.changed() => {
                    match self.handle_state_change(&mut permit, &mut state_receiver) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                }
            }
        }
    }

    async fn download_chapter(&self) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;
        if let Err(err) = self.comic.save_metadata(&self.app) {
            let err_title = format!("`{comic_title}`保存元数据失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }
        // 获取章节图片URL列表
        let Some(url_and_index_pairs) = self.get_url_and_index_pairs().await else {
            return;
        };
        // 记录总共需要下载的图片数量
        #[allow(clippy::cast_possible_truncation)]
        self.total_img_count
            .fetch_add(url_and_index_pairs.len() as u32, Ordering::Relaxed);
        // 创建临时下载目录
        let Some(temp_download_dir) = self.create_temp_download_dir() else {
            return;
        };
        // 清理临时下载目录中与`config.download_format`对不上的文件
        self.clean_temp_download_dir(&temp_download_dir);

        let mut join_set = JoinSet::new();
        for (url, index) in url_and_index_pairs {
            let url = url.clone();
            let temp_download_dir = temp_download_dir.clone();
            // 创建下载任务
            let download_img_task = DownloadImgTask::new(self, url, index, temp_download_dir);
            join_set.spawn(download_img_task.process());
        }
        join_set.join_all().await;
        tracing::trace!(comic_title, chapter_title, "所有图片下载任务完成");
        // 如果DownloadManager所有图片全部都已下载(无论成功或失败)，则清空下载进度
        let downloaded_img_count = self.downloaded_img_count.load(Ordering::Relaxed);
        let total_img_count = self.total_img_count.load(Ordering::Relaxed);
        if downloaded_img_count != total_img_count {
            // 此章节的图片未全部下载成功
            let err_title = format!("`{comic_title} - {chapter_title}`下载不完整");
            let err_msg =
                format!("总共有`{total_img_count}`张图片，但只下载了`{downloaded_img_count}`张");
            tracing::error!(err_title, message = err_msg);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }

        if let Err(err) = self.rename_temp_download_dir(&temp_download_dir) {
            let err_title = format!("`{comic_title} - {chapter_title}`保存下载目录失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }

        tracing::info!(comic_title, chapter_title, "章节下载成功");

        self.set_state(DownloadTaskState::Completed);
        self.emit_download_task_update_event();
    }

    async fn get_url_and_index_pairs(&self) -> Option<Vec<(String, i64)>> {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        let chapter_resp_data = match self.get_chapter_with_retry().await {
            Ok(data) => data,
            Err(err) => {
                let err_title = format!("获取漫画 {comic_title} 的 {chapter_title} 信息失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);

                self.set_state(DownloadTaskState::Failed);
                self.emit_download_task_update_event();

                return None;
            }
        };

        let urls: Vec<String> = chapter_resp_data
            .chapter
            .contents
            .into_iter()
            .map(|content| content.url.replace(".c800x.", ".c1500x."))
            .collect();

        let url_and_index_pairs: Vec<(String, i64)> = urls
            .into_iter()
            .enumerate()
            .map(|(i, url)| {
                let index = chapter_resp_data.chapter.words[i];
                (url, index)
            })
            .collect();

        Some(url_and_index_pairs)
    }

    fn create_temp_download_dir(&self) -> Option<PathBuf> {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        let temp_download_dir = self.get_temp_download_dir();

        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(anyhow::Error::from) {
            let err_title = format!(
                "`{comic_title} - {chapter_title}`创建临时下载目录`{}`失败",
                temp_download_dir.display()
            );
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return None;
        }

        tracing::trace!(
            comic_title,
            chapter_title,
            "创建临时下载目录`{}`成功",
            temp_download_dir.display()
        );

        Some(temp_download_dir)
    }

    async fn get_chapter_with_retry(&self) -> anyhow::Result<GetChapterRespData> {
        let comic_path_word = &self.chapter_info.comic_path_word;
        let chapter_uuid = &self.chapter_info.chapter_uuid;

        let copy_client = self.copy_client();
        let mut retry_count = 0;
        loop {
            match copy_client.get_chapter(comic_path_word, chapter_uuid).await {
                Ok(data) => return Ok(data),
                Err(CopyMangaError::Anyhow(err)) => return Err(err),
                Err(CopyMangaError::RiskControl(RiskControlError::Register(_))) => {
                    const RETRY_WAIT_TIME: u32 = 60;
                    for i in 1..=RETRY_WAIT_TIME {
                        let _ = DownloadControlRiskEvent {
                            chapter_uuid: chapter_uuid.clone(),
                            retry_after: RETRY_WAIT_TIME - i,
                        }
                        .emit(&self.app);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
                Err(err) => {
                    // 随机等待1000-5000ms
                    let wait_time = 1000 + rand::random::<u64>() % 4000;
                    tokio::time::sleep(Duration::from_millis(wait_time)).await;
                    if retry_count < 5 {
                        retry_count += 1;
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }

    /// 删除临时下载目录中与`config.download_format`对不上的文件
    fn clean_temp_download_dir(&self, temp_download_dir: &Path) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        let entries = match std::fs::read_dir(temp_download_dir).map_err(anyhow::Error::from) {
            Ok(entries) => entries,
            Err(err) => {
                let err_title = format!(
                    "`{comic_title}`读取临时下载目录`{}`失败",
                    temp_download_dir.display()
                );
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return;
            }
        };

        let download_format = self.app.state::<RwLock<Config>>().read().download_format;
        let extension = download_format.extension();
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            // path有扩展名，且能转换为utf8，并与`config.download_format`一致或是gif，则保留
            let should_keep = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == extension);
            if should_keep {
                continue;
            }
            // 否则删除文件
            if let Err(err) = std::fs::remove_file(&path).map_err(anyhow::Error::from) {
                let err_title =
                    format!("`{comic_title}`删除临时下载目录的`{}`失败", path.display());
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
            }
        }

        tracing::trace!(
            comic_title,
            chapter_title,
            "清理临时下载目录`{}`成功",
            temp_download_dir.display()
        );
    }

    fn rename_temp_download_dir(&self, temp_download_dir: &PathBuf) -> anyhow::Result<()> {
        let Some(parent) = temp_download_dir.parent() else {
            return Err(anyhow!("无法获取 {temp_download_dir:?} 的父目录"));
        };

        let download_dir = parent.join(&self.chapter_info.prefixed_chapter_title);

        if download_dir.exists() {
            std::fs::remove_dir_all(&download_dir)
                .context(format!("删除 {download_dir:?} 失败"))?;
        }

        std::fs::rename(temp_download_dir, &download_dir).context(format!(
            "将 {temp_download_dir:?} 重命名为 {download_dir:?} 失败"
        ))?;

        Ok(())
    }

    fn get_temp_download_dir(&self) -> PathBuf {
        let comic_title = &self.chapter_info.comic_title;
        let group_name = &self.chapter_info.group_name;
        let prefixed_chapter_title = &self.chapter_info.prefixed_chapter_title;

        self.app
            .state::<RwLock<Config>>()
            .read()
            .download_dir
            .join(comic_title)
            .join(group_name)
            .join(format!(".下载中-{prefixed_chapter_title}")) // 以 `.下载中-` 开头，表示是临时目录
    }

    async fn acquire_chapter_permit<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
    ) -> ControlFlow<()> {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        tracing::debug!(comic_title, chapter_title, "章节开始排队");

        self.emit_download_task_update_event();

        *permit = match permit.take() {
            // 如果有permit，则直接用
            Some(permit) => Some(permit),
            // 如果没有permit，则获取permit
            None => match self
                .download_manager
                .chapter_sem
                .acquire()
                .await
                .map_err(anyhow::Error::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title =
                        format!("`{comic_title} - {chapter_title}`获取下载章节的permit失败");
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);

                    self.set_state(DownloadTaskState::Failed);
                    self.emit_download_task_update_event();

                    return ControlFlow::Break(());
                }
            },
        };
        // 如果当前任务状态不是`Pending`，则不将任务状态设置为`Downloading`
        if *self.state_sender.borrow() != DownloadTaskState::Pending {
            return ControlFlow::Continue(());
        }
        // 将任务状态设置为`Downloading`
        if let Err(err) = self
            .state_sender
            .send(DownloadTaskState::Downloading)
            .map_err(anyhow::Error::from)
        {
            let err_title = format!("`{comic_title} - {chapter_title}`发送状态`Downloading`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) -> ControlFlow<()> {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        self.emit_download_task_update_event();
        let state = *state_receiver.borrow();
        match state {
            DownloadTaskState::Paused => {
                tracing::debug!(comic_title, chapter_title, "章节暂停中");
                if let Some(permit) = permit.take() {
                    drop(permit);
                }
                ControlFlow::Continue(())
            }
            DownloadTaskState::Cancelled => {
                tracing::debug!(comic_title, chapter_title, "章节取消下载");
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    }

    fn set_state(&self, state: DownloadTaskState) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        if let Err(err) = self.state_sender.send(state).map_err(anyhow::Error::from) {
            let err_title = format!("`{comic_title} - {chapter_title}`发送状态`{state:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }
    }

    fn emit_download_task_update_event(&self) {
        let _ = DownloadTaskEvent::Update {
            chapter_uuid: self.chapter_info.chapter_uuid.clone(),
            state: *self.state_sender.borrow(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn emit_download_task_create_event(&self) {
        let _ = DownloadTaskEvent::Create {
            state: *self.state_sender.borrow(),
            comic: Box::new(self.comic.as_ref().clone()),
            chapter_info: Box::new(self.chapter_info.as_ref().clone()),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn copy_client(&self) -> CopyClient {
        self.app.state::<CopyClient>().inner().clone()
    }
}

#[derive(Clone)]
struct DownloadImgTask {
    app: AppHandle,
    download_manager: DownloadManager,
    download_task: DownloadTask,
    url: String,
    index: i64,
    temp_download_dir: PathBuf,
}

impl DownloadImgTask {
    pub fn new(
        download_task: &DownloadTask,
        url: String,
        index: i64,
        temp_download_dir: PathBuf,
    ) -> Self {
        DownloadImgTask {
            app: download_task.app.clone(),
            download_manager: download_task.download_manager.clone(),
            download_task: download_task.clone(),
            url,
            index,
            temp_download_dir,
        }
    }

    async fn process(self) {
        let download_img_task = self.download_img();
        tokio::pin!(download_img_task);

        let mut state_receiver = self.download_task.state_sender.subscribe();
        state_receiver.mark_changed();
        let mut permit = None;

        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            tokio::select! {
                () = &mut download_img_task, if state_is_downloading && permit.is_some() => break,
                control_flow = self.acquire_img_permit(&mut permit), if state_is_downloading && permit.is_none() => {
                    match control_flow {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                },
                _ = state_receiver.changed() => {
                    match self.handle_state_change(&mut permit, &mut state_receiver) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                }
            }
        }
    }

    async fn download_img(&self) {
        let url = &self.url;
        let comic_title = &self.download_task.comic.comic.name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        let download_format = self.app.state::<RwLock<Config>>().read().download_format;
        let extension = download_format.extension();
        let save_path = self
            .temp_download_dir
            .join(format!("{:03}.{extension}", self.index + 1));
        if save_path.exists() {
            // 如果图片已经存在，则直接跳过下载
            self.download_task
                .downloaded_img_count
                .fetch_add(1, Ordering::Relaxed);

            self.download_task.emit_download_task_update_event();

            tracing::trace!(url, comic_title, chapter_title, "图片已存在，跳过下载");
            return;
        }

        tracing::trace!(url, comic_title, chapter_title, "开始下载图片");

        let (img_data, img_format) = match self.copy_client().get_img_data_and_format(url).await {
            Ok(data_and_format) => data_and_format,
            Err(err) => {
                let err_title = format!("下载图片`{url}`失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return;
            }
        };
        let img_data_len = img_data.len() as u64;

        tracing::trace!(url, comic_title, chapter_title, "图片成功下载到内存");

        // 保存图片
        let target_format = download_format.to_image_format();
        if let Err(err) = save_img(&save_path, target_format, &img_data, img_format) {
            let err_title = format!("保存图片`{url}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }

        tracing::trace!(
            url,
            comic_title,
            chapter_title,
            "图片成功保存到`{}`",
            save_path.display()
        );

        // 记录下载字节数
        self.download_manager
            .byte_per_sec
            .fetch_add(img_data_len, Ordering::Relaxed);

        self.download_task
            .downloaded_img_count
            .fetch_add(1, Ordering::Relaxed);

        self.download_task.emit_download_task_update_event();
    }

    async fn acquire_img_permit<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
    ) -> ControlFlow<()> {
        let url = &self.url;
        let comic_title = &self.download_task.comic.comic.name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        tracing::trace!(comic_title, chapter_title, url, "图片开始排队");

        *permit = match permit.take() {
            // 如果有permit，则直接用
            Some(permit) => Some(permit),
            // 如果没有permit，则获取permit
            None => match self
                .download_manager
                .img_sem
                .acquire()
                .await
                .map_err(anyhow::Error::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title =
                        format!("`{comic_title} - {chapter_title}`获取下载图片的permit失败");
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    return ControlFlow::Break(());
                }
            },
        };
        ControlFlow::Continue(())
    }

    fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) -> ControlFlow<()> {
        let url = &self.url;
        let comic_title = &self.download_task.comic.comic.name;
        let chapter_title = &self.download_task.chapter_info.chapter_title;

        let state = *state_receiver.borrow();
        match state {
            DownloadTaskState::Paused => {
                tracing::trace!(comic_title, chapter_title, url, "图片暂停下载");
                if let Some(permit) = permit.take() {
                    drop(permit);
                }
                ControlFlow::Continue(())
            }
            DownloadTaskState::Cancelled => {
                tracing::trace!(comic_title, chapter_title, url, "图片取消下载");
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    }

    fn copy_client(&self) -> CopyClient {
        self.app.state::<CopyClient>().inner().clone()
    }
}

fn save_img(
    save_path: &Path,
    target_format: ImageFormat,
    src_img_data: &Bytes,
    src_format: ImageFormat,
) -> anyhow::Result<()> {
    if target_format == src_format {
        // 如果target_format与src_format匹配，则直接保存
        std::fs::write(save_path, src_img_data)
            .context(format!("将图片数据写入`{save_path:?}`失败"))?;
        return Ok(());
    }
    // 如果target_format与src_format不匹配，则需要转换格式
    let img = image::load_from_memory(src_img_data).context("加载图片数据失败")?;

    let mut converted_data = Vec::new();
    match target_format {
        ImageFormat::WebP => img
            .to_rgba8()
            .write_to(&mut Cursor::new(&mut converted_data), ImageFormat::WebP),
        ImageFormat::Jpeg => img
            .to_rgb8()
            .write_to(&mut Cursor::new(&mut converted_data), ImageFormat::Jpeg),
        _ => return Err(anyhow!("不支持的图片格式: {:?}", target_format)),
    }
    .context(format!("将`{src_format:?}`转换为`{target_format:?}`失败"))?;

    std::fs::write(save_path, &converted_data)
        .context(format!("将图片数据写入`{save_path:?}`失败"))?;

    Ok(())
}
