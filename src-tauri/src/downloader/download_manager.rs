use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use eyre::{eyre, WrapErr};
use parking_lot::RwLock;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::Semaphore;

use crate::{
    downloader::{download_task::DownloadTask, download_task_state::DownloadTaskState},
    events::DownloadEvent,
    extensions::{AppHandleExt, EyreReportToMessage},
    types::Comic,
};

pub struct DownloadManager {
    pub app: AppHandle,
    pub chapter_sem: Arc<Semaphore>,
    pub img_sem: Arc<Semaphore>,
    pub byte_per_sec: Arc<AtomicU64>,
    pub download_tasks: RwLock<HashMap<String, Arc<DownloadTask>>>,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let (chapter_concurrency, img_concurrency) = {
            let config = app.get_config();
            let config = config.read();
            (config.chapter_concurrency, config.img_concurrency)
        };

        let manager = DownloadManager {
            app: app.clone(),
            chapter_sem: Arc::new(Semaphore::new(chapter_concurrency)),
            img_sem: Arc::new(Semaphore::new(img_concurrency)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: RwLock::new(HashMap::new()),
        };

        tauri::async_runtime::spawn(Self::emit_download_speed_loop(
            manager.app.clone(),
            manager.byte_per_sec.clone(),
        ));

        manager
    }

    async fn emit_download_speed_loop(app: AppHandle, byte_per_sec: Arc<AtomicU64>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let byte_per_sec = byte_per_sec.swap(0, Ordering::Relaxed);
            #[allow(clippy::cast_precision_loss)]
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2}MB/s");
            let _ = DownloadEvent::Speed { speed }.emit(&app);
        }
    }

    pub fn create_download_tasks(&self, mut comic: Comic, chapter_uuids: &[String]) {
        use DownloadTaskState::{Downloading, Paused, Pending};

        let _ = comic.ensure_download_dir_fields(&self.app);

        let comic_title = &comic.comic.name;
        let mut tasks = self.download_tasks.write();
        for chapter_uuid in chapter_uuids {
            if let Some(task) = tasks.get(chapter_uuid) {
                // 如果任务已经存在，且状态是`Pending`、`Downloading`或`Paused`，则不创建新任务
                let state = *task.state_sender.borrow();
                if matches!(state, Pending | Downloading | Paused) {
                    let err = eyre!("章节ID为`{chapter_uuid}`的下载任务已存在");
                    let err_title =
                        format!("`{comic_title}`的章节ID为`{chapter_uuid}`的下载任务创建失败");
                    let message = err.to_message();
                    tracing::error!(err_title, message);
                    continue;
                }
            }

            if let Some(task) = tasks.remove(chapter_uuid) {
                if let Err(err) = task
                    .delete_sender
                    .send(())
                    .context(format!("章节ID为`{chapter_uuid}`的下载任务删除失败"))
                {
                    let err_title = "章节ID对应的下载任务创建失败";
                    let message = err.to_message();
                    tracing::error!(err_title, message);
                    continue;
                }
            }

            let task = match DownloadTask::new(self.app.clone(), comic.clone(), chapter_uuid) {
                Ok(task) => task,
                Err(err) => {
                    let err_title =
                        format!("`{comic_title}`的章节ID为`{chapter_uuid}`的下载任务创建失败");
                    let message = err.to_message();
                    tracing::error!(err_title, message);
                    continue;
                }
            };

            tasks.insert(chapter_uuid.clone(), task);

            tracing::debug!("创建章节ID为`{chapter_uuid}`的下载任务成功");
        }
    }

    pub fn pause_download_task(&self, chapter_uuid: &str) -> eyre::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(chapter_uuid) else {
            return Err(eyre!("未找到章节ID为`{chapter_uuid}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Paused);
        Ok(())
    }

    pub fn resume_download_task(&self, chapter_uuid: &str) -> eyre::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(chapter_uuid) else {
            return Err(eyre!("未找到章节ID为`{chapter_uuid}`的下载任务"));
        };
        task.set_state(DownloadTaskState::Pending);
        Ok(())
    }

    pub fn delete_download_task(&self, chapter_uuid: &str) -> eyre::Result<()> {
        let mut tasks = self.download_tasks.write();
        let Some(task) = tasks.remove(chapter_uuid) else {
            return Err(eyre!("未找到章节ID为`{chapter_uuid}`的下载任务"));
        };
        task.delete_sender
            .send(())
            .wrap_err(format!("通知章节ID为`{chapter_uuid}`的下载任务删除失败"))?;
        Ok(())
    }
}
