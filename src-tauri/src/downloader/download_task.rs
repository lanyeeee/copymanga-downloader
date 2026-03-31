use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Context;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::{
    sync::{watch, SemaphorePermit},
    task::JoinSet,
    time::sleep,
};

use crate::{
    downloader::{download_img_task::DownloadImgTask, download_task_state::DownloadTaskState},
    errors::RiskControlError,
    events::DownloadEvent,
    extensions::{AnyhowErrorToStringChain, AppHandleExt},
    responses::GetChapterRespData,
    types::{ChapterInfo, Comic},
};

pub struct DownloadTask {
    pub app: AppHandle,
    pub comic: Arc<Comic>,
    pub chapter_info: Arc<ChapterInfo>,
    pub state_sender: watch::Sender<DownloadTaskState>,
    pub delete_sender: watch::Sender<()>,
    pub downloaded_img_count: Arc<AtomicU32>,
    pub total_img_count: Arc<AtomicU32>,
}

impl DownloadTask {
    pub fn new(app: AppHandle, mut comic: Comic, chapter_uuid: &str) -> anyhow::Result<Arc<Self>> {
        comic.ensure_download_dir_fields(&app).context(format!(
            "漫画`{}`更新`download_dir`字段失败",
            comic.comic.name
        ))?;

        let chapter_info = comic
            .comic
            .groups
            .iter()
            .flat_map(|(_, chapter_infos)| chapter_infos.iter())
            .find(|chapter_info| chapter_info.chapter_uuid == chapter_uuid)
            .cloned()
            .context(format!("未找到章节ID为`{chapter_uuid}`的章节信息"))?;

        let (state_sender, _) = watch::channel(DownloadTaskState::Pending);
        let (delete_sender, _) = watch::channel(());

        let task = Arc::new(Self {
            app,
            comic: Arc::new(comic),
            chapter_info: Arc::new(chapter_info),
            state_sender,
            delete_sender,
            downloaded_img_count: Arc::new(AtomicU32::new(0)),
            total_img_count: Arc::new(AtomicU32::new(0)),
        });

        tauri::async_runtime::spawn(task.clone().process());

        Ok(task)
    }

    async fn process(self: Arc<Self>) {
        self.emit_download_task_create_event();

        let mut state_receiver = self.state_sender.subscribe();
        state_receiver.mark_changed();

        let mut delete_receiver = self.delete_sender.subscribe();

        let mut permit = None;
        let mut download_task_option = None;

        loop {
            let state = *state_receiver.borrow();
            let state_is_downloading = state == DownloadTaskState::Downloading;
            let state_is_pending = state == DownloadTaskState::Pending;

            let download_task = async {
                download_task_option
                    .get_or_insert_with(|| Box::pin(self.download_chapter()))
                    .await;
            };

            tokio::select! {
                () = download_task, if state_is_downloading && permit.is_some() => {
                    download_task_option = None;
                    if let Some(permit) = permit.take() {
                        drop(permit);
                    };
                }

                () = self.acquire_chapter_permit(&mut permit), if state_is_pending => {},

                _ = state_receiver.changed() => {
                    self.handle_state_change(&mut permit, &mut state_receiver).await;
                }

                _ = delete_receiver.changed() => {
                    self.handle_delete_receiver_change(&mut permit).await;
                    return;
                }
            }
        }
    }

    async fn download_chapter(self: &Arc<Self>) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;
        if let Err(err) = self.comic.save_metadata() {
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
            let download_img_task =
                DownloadImgTask::new(self.clone(), url, index, temp_download_dir);
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

        if let Err(err) = self.chapter_info.save_metadata() {
            let err_title = format!("`{comic_title} - {chapter_title}`保存章节元数据失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }

        tracing::info!(comic_title, chapter_title, "章节下载成功");
        self.sleep_between_chapter().await;

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

        let temp_download_dir = match self.chapter_info.get_temp_download_dir() {
            Ok(temp_download_dir) => temp_download_dir,
            Err(err) => {
                let err_title = format!("`{comic_title} - {chapter_title}`获取临时下载目录失败");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);

                self.set_state(DownloadTaskState::Failed);
                self.emit_download_task_update_event();

                return None;
            }
        };

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

        let copy_client = self.app.get_copy_client();
        loop {
            match copy_client.get_chapter(comic_path_word, chapter_uuid).await {
                Ok(data) => return Ok(data),
                Err(RiskControlError::Anyhow(err)) => return Err(err),
                Err(RiskControlError::RiskControl(_)) => {
                    const RETRY_WAIT_TIME: u32 = 60;
                    for i in 1..=RETRY_WAIT_TIME {
                        let _ = DownloadEvent::RiskControl {
                            chapter_uuid: chapter_uuid.clone(),
                            retry_after: RETRY_WAIT_TIME - i,
                        }
                        .emit(&self.app);
                        sleep(Duration::from_secs(1)).await;
                    }
                    self.emit_download_task_update_event();
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

        let download_format = self.app.get_config().read().download_format;
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
        let chapter_download_dir = self
            .chapter_info
            .chapter_download_dir
            .as_ref()
            .context("`chapter_download_dir`字段为`None`")?;

        if chapter_download_dir.exists() {
            std::fs::remove_dir_all(chapter_download_dir)
                .context(format!("删除`{}`失败", chapter_download_dir.display()))?;
        }

        std::fs::rename(temp_download_dir, chapter_download_dir).context(format!(
            "将`{}`重命名为`{}`失败",
            temp_download_dir.display(),
            chapter_download_dir.display()
        ))?;

        Ok(())
    }

    async fn acquire_chapter_permit<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        tracing::debug!(comic_title, chapter_title, "章节开始排队");

        self.emit_download_task_update_event();

        *permit = match permit.take() {
            // 如果有permit，则直接用
            Some(permit) => Some(permit),
            // 如果没有permit，则获取permit
            None => match self
                .app
                .get_download_manager()
                .inner()
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
                    return;
                }
            },
        };
        // 如果当前任务状态不是`Pending`，则不将任务状态设置为`Downloading`
        if *self.state_sender.borrow() != DownloadTaskState::Pending {
            return;
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

            self.set_state(DownloadTaskState::Failed);
        }
    }

    async fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        self.emit_download_task_update_event();
        let state = *state_receiver.borrow();

        if state == DownloadTaskState::Paused {
            // 稍微等一下再释放permit
            // 避免大批量暂停时，本应暂停的任务因拿到permit而稍微下载一小段(虽然最终会被暂停)
            sleep(Duration::from_millis(100)).await;
            tracing::debug!(comic_title, chapter_title, "下载任务已暂停");
            if let Some(permit) = permit.take() {
                drop(permit);
            };
        } else if state == DownloadTaskState::Failed {
            // 稍微等一下再释放permit
            // 避免大批量失败时，本应失败的任务因拿到permit而稍微下载一小段(虽然最终会被标记为失败)
            sleep(Duration::from_millis(100)).await;
            if let Some(permit) = permit.take() {
                drop(permit);
            };
        }
    }

    async fn handle_delete_receiver_change<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;
        let chapter_uuid = self.chapter_info.chapter_uuid.clone();

        let _ = DownloadEvent::TaskDelete { chapter_uuid }.emit(&self.app);

        if permit.is_some() {
            // 如果有permit则稍微等一下再退出
            // 这是为了避免大批量删除时，本应删除的任务因拿到permit而又稍微下载一小段
            sleep(Duration::from_millis(100)).await;
        }

        tracing::debug!(comic_title, chapter_title, "下载任务已删除");
    }

    async fn sleep_between_chapter(&self) {
        let mut remaining_sec = self.app.get_config().read().chapter_download_interval_sec;
        while remaining_sec > 0 {
            // 发送章节休眠事件
            let _ = DownloadEvent::Sleeping {
                chapter_uuid: self.chapter_info.chapter_uuid.clone(),
                remaining_sec,
            }
            .emit(&self.app);
            sleep(Duration::from_secs(1)).await;
            remaining_sec -= 1;
        }
    }

    pub fn set_state(&self, state: DownloadTaskState) {
        let comic_title = &self.comic.comic.name;
        let chapter_title = &self.chapter_info.chapter_title;

        if let Err(err) = self.state_sender.send(state).map_err(anyhow::Error::from) {
            let err_title = format!("`{comic_title} - {chapter_title}`发送状态`{state:?}`失败");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }
    }

    pub fn emit_download_task_update_event(&self) {
        let _ = DownloadEvent::TaskUpdate {
            chapter_uuid: self.chapter_info.chapter_uuid.clone(),
            state: *self.state_sender.borrow(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn emit_download_task_create_event(&self) {
        let _ = DownloadEvent::TaskCreate {
            state: *self.state_sender.borrow(),
            comic: Box::new(self.comic.as_ref().clone()),
            chapter_info: Box::new(self.chapter_info.as_ref().clone()),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }
}
