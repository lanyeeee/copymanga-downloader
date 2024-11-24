use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::{
    sync::{mpsc, Semaphore},
    task::JoinSet,
};

use crate::{
    config::Config,
    copy_client::CopyClient,
    errors::{CopyMangaError, RiskControlError},
    events::DownloadEvent,
    extensions::AnyhowErrorToStringChain,
    responses::GetChapterRespData,
    types::ChapterInfo,
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
    sender: Arc<mpsc::Sender<ChapterInfo>>,
    ep_sem: Arc<Semaphore>,
    img_sem: Arc<Semaphore>,
    byte_per_sec: Arc<AtomicU64>,
    downloaded_image_count: Arc<AtomicU32>,
    total_image_count: Arc<AtomicU32>,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel::<ChapterInfo>(32);

        let manager = DownloadManager {
            app: app.clone(),
            sender: Arc::new(sender),
            ep_sem: Arc::new(Semaphore::new(3)),
            img_sem: Arc::new(Semaphore::new(30)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            downloaded_image_count: Arc::new(AtomicU32::new(0)),
            total_image_count: Arc::new(AtomicU32::new(0)),
        };

        tauri::async_runtime::spawn(Self::log_download_speed(app.clone()));
        tauri::async_runtime::spawn(Self::receiver_loop(app.clone(), receiver));

        manager
    }

    pub async fn submit_chapter(&self, chapter_info: ChapterInfo) -> anyhow::Result<()> {
        self.sender.send(chapter_info).await?;
        Ok(())
    }

    #[allow(clippy::cast_precision_loss)]
    async fn log_download_speed(app: AppHandle) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let manager = app.state::<DownloadManager>();
            let byte_per_sec = manager.byte_per_sec.swap(0, Ordering::Relaxed);
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");
            // 发送总进度条下载速度事件
            let _ = DownloadEvent::OverallSpeed { speed }.emit(&app);
        }
    }

    async fn receiver_loop(app: AppHandle, mut receiver: mpsc::Receiver<ChapterInfo>) {
        while let Some(chapter_info) = receiver.recv().await {
            let manager = app.state::<DownloadManager>().inner().clone();
            tauri::async_runtime::spawn(manager.process_chapter(chapter_info));
        }
    }

    // TODO: 这里不应该返回错误，否则会被忽略
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_lossless)]
    #[allow(clippy::too_many_lines)] // TODO: 重构，减少函数长度
    async fn process_chapter(self, chapter_info: ChapterInfo) {
        // 发送下载章节排队事件
        let _ = DownloadEvent::ChapterPending {
            chapter_uuid: chapter_info.chapter_uuid.clone(),
            comic_title: chapter_info.comic_title.clone(),
            chapter_title: chapter_info.chapter_title.clone(),
        }
        .emit(&self.app);

        let temp_download_dir = get_temp_download_dir(&self.app, &chapter_info);

        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(anyhow::Error::from) {
            // 如果创建目录失败，则发送下载章节结束事件，并返回
            let err = err.context(format!("创建目录 {temp_download_dir:?} 失败"));
            // 发送下载章节结束事件
            let _ = DownloadEvent::ChapterEnd {
                chapter_uuid: chapter_info.chapter_uuid.clone(),
                err_msg: Some(err.to_string_chain()),
            }
            .emit(&self.app);
            return;
        }

        let chapter_resp_data = match self.get_chapter_resp_data(&chapter_info).await {
            Ok(data) => data,
            Err(err) => {
                let comic_title = &chapter_info.comic_title;
                let chapter_title = &chapter_info.chapter_title;
                let err = err.context(format!(
                    "获取漫画 {comic_title} 的 {chapter_title} 信息失败"
                ));
                // 发送下载章节结束事件
                let _ = DownloadEvent::ChapterEnd {
                    chapter_uuid: chapter_info.chapter_uuid.clone(),
                    err_msg: Some(err.to_string_chain()),
                }
                .emit(&self.app);
                return;
            }
        };
        let urls: Vec<String> = chapter_resp_data
            .chapter
            .contents
            .into_iter()
            .map(|content| content.url.replace(".c800x.", ".c1500x."))
            .collect();
        let total = urls.len() as u32;
        // 记录总共需要下载的图片数量
        self.total_image_count.fetch_add(total, Ordering::Relaxed);
        let current = Arc::new(AtomicU32::new(0));
        let mut join_set = JoinSet::new();
        // 限制同时下载的章节数量
        let permit = match self.ep_sem.acquire().await.map_err(anyhow::Error::from) {
            Ok(permit) => permit,
            Err(err) => {
                let err = err.context("获取下载章节的semaphore失败");
                // 发送下载章节结束事件
                let _ = DownloadEvent::ChapterEnd {
                    chapter_uuid: chapter_info.chapter_uuid.clone(),
                    err_msg: Some(err.to_string_chain()),
                }
                .emit(&self.app);
                return;
            }
        };
        // 发送下载章节开始事件
        let _ = DownloadEvent::ChapterStart {
            chapter_uuid: chapter_info.chapter_uuid.clone(),
            total,
        }
        .emit(&self.app);
        // 逐一下载图片
        for (i, url) in urls.iter().enumerate() {
            let manager = self.clone();
            let url = url.clone();
            let save_path = temp_download_dir.join(format!("{:03}.jpg", i + 1));
            let ep_id = chapter_info.chapter_uuid.clone();
            let current = current.clone();
            // 创建下载任务
            join_set.spawn(manager.download_image(url, save_path, ep_id, current));
        }
        // 逐一处理完成的下载任务
        while let Some(Ok(())) = join_set.join_next().await {
            self.downloaded_image_count.fetch_add(1, Ordering::Relaxed);
            let downloaded_image_count = self.downloaded_image_count.load(Ordering::Relaxed);
            let total_image_count = self.total_image_count.load(Ordering::Relaxed);
            // 更新下载进度
            let percentage = downloaded_image_count as f64 / total_image_count as f64 * 100.0;
            // 发送总进度条更新事件
            let _ = DownloadEvent::OverallUpdate {
                downloaded_image_count,
                total_image_count,
                percentage,
            }
            .emit(&self.app);
        }
        drop(permit);
        // 如果DownloadManager所有图片全部都已下载(无论成功或失败)，则清空下载进度
        let downloaded_image_count = self.downloaded_image_count.load(Ordering::Relaxed);
        let total_image_count = self.total_image_count.load(Ordering::Relaxed);
        if downloaded_image_count == total_image_count {
            self.downloaded_image_count.store(0, Ordering::Relaxed);
            self.total_image_count.store(0, Ordering::Relaxed);
        }
        // 检查此章节的图片是否全部下载成功
        let current = current.load(Ordering::Relaxed);
        // 此章节的图片未全部下载成功
        if current != total {
            let err_msg = Some(format!("总共有 {total} 张图片，但只下载了 {current} 张"));
            // 发送下载章节结束事件
            let _ = DownloadEvent::ChapterEnd {
                chapter_uuid: chapter_info.chapter_uuid.clone(),
                err_msg,
            }
            .emit(&self.app);
            return;
        }
        // 此章节的图片全部下载成功
        let err_msg = match self.save_archive(&chapter_info, &temp_download_dir) {
            Ok(()) => None,
            Err(err) => Some(err.to_string_chain()),
        };
        // 发送下载章节结束事件
        let _ = DownloadEvent::ChapterEnd {
            chapter_uuid: chapter_info.chapter_uuid.clone(),
            err_msg,
        }
        .emit(&self.app);
    }

    async fn get_chapter_resp_data(
        &self,
        chapter_info: &ChapterInfo,
    ) -> anyhow::Result<GetChapterRespData> {
        let copy_client = self.copy_client();
        let mut retry_count = 0;
        loop {
            match copy_client
                .get_chapter(&chapter_info.comic_path_word, &chapter_info.chapter_uuid)
                .await
            {
                Ok(data) => return Ok(data),
                Err(CopyMangaError::Anyhow(err)) => return Err(err),
                Err(CopyMangaError::RiskControl(RiskControlError::Register(_))) => {
                    const RETRY_WAIT_TIME: u32 = 60;
                    for i in 1..=RETRY_WAIT_TIME {
                        let _ = DownloadEvent::ChapterControlRisk {
                            chapter_uuid: chapter_info.chapter_uuid.clone(),
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

    async fn download_image(
        self,
        url: String,
        save_path: PathBuf,
        chapter_uuid: String,
        current: Arc<AtomicU32>,
    ) {
        // 下载图片
        let permit = match self.img_sem.acquire().await.map_err(anyhow::Error::from) {
            Ok(permit) => permit,
            Err(err) => {
                let err = err.context("获取下载图片的semaphore失败");
                // 发送下载图片失败事件
                let _ = DownloadEvent::ImageError {
                    chapter_uuid: chapter_uuid.clone(),
                    url: url.clone(),
                    err_msg: err.to_string_chain(),
                }
                .emit(&self.app);
                return;
            }
        };
        let image_data = match self.copy_client().get_image_bytes(&url).await {
            Ok(data) => data,
            Err(err) => {
                let err = err.context(format!("下载图片 {url} 失败"));
                // 发送下载图片失败事件
                let _ = DownloadEvent::ImageError {
                    chapter_uuid: chapter_uuid.clone(),
                    url: url.clone(),
                    err_msg: err.to_string_chain(),
                }
                .emit(&self.app);
                return;
            }
        };
        drop(permit);
        // 保存图片
        if let Err(err) = std::fs::write(&save_path, &image_data).map_err(anyhow::Error::from) {
            let err = err.context(format!("保存图片 {save_path:?} 失败"));
            // 发送下载图片失败事件
            let _ = DownloadEvent::ImageError {
                chapter_uuid: chapter_uuid.clone(),
                url: url.clone(),
                err_msg: err.to_string_chain(),
            }
            .emit(&self.app);
            return;
        }
        // 记录下载字节数
        self.byte_per_sec
            .fetch_add(image_data.len() as u64, Ordering::Relaxed);
        // 更新章节下载进度
        let current = current.fetch_add(1, Ordering::Relaxed) + 1;
        // 发送下载图片成功事件
        let _ = DownloadEvent::ImageSuccess {
            chapter_uuid: chapter_uuid.clone(),
            url,
            current,
        }
        .emit(&self.app);
    }

    fn save_archive(
        &self,
        chapter_info: &ChapterInfo,
        temp_download_dir: &PathBuf,
    ) -> anyhow::Result<()> {
        let Some(parent) = temp_download_dir.parent() else {
            return Err(anyhow!("无法获取 {temp_download_dir:?} 的父目录"));
        };

        let download_dir = parent.join(&chapter_info.chapter_title);

        if download_dir.exists() {
            std::fs::remove_dir_all(&download_dir)
                .context(format!("删除 {download_dir:?} 失败"))?;
        }

        std::fs::rename(temp_download_dir, &download_dir).context(format!(
            "将 {temp_download_dir:?} 重命名为 {download_dir:?} 失败"
        ))?;

        Ok(())
    }

    fn copy_client(&self) -> CopyClient {
        self.app.state::<CopyClient>().inner().clone()
    }
}

fn get_temp_download_dir(app: &AppHandle, chapter_info: &ChapterInfo) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .download_dir
        .join(&chapter_info.comic_title)
        .join(&chapter_info.group_name)
        .join(format!(".下载中-{}", chapter_info.chapter_title)) // 以 `.下载中-` 开头，表示是临时目录
}
