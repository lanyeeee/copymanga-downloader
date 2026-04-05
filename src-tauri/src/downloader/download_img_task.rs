use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use bytes::Bytes;
use eyre::{eyre, WrapErr};
use image::ImageFormat;
use tauri::AppHandle;
use tokio::{
    sync::{watch, SemaphorePermit},
    time::sleep,
};
use tracing::instrument;

use crate::{
    downloader::{download_task::DownloadTask, download_task_state::DownloadTaskState},
    extensions::{AppHandleExt, EyreReportToMessage},
};

pub struct DownloadImgTask {
    app: AppHandle,
    download_task: Arc<DownloadTask>,
    url: String,
    index: i64,
    temp_download_dir: PathBuf,
}

impl DownloadImgTask {
    pub fn new(
        download_task: Arc<DownloadTask>,
        url: String,
        index: i64,
        temp_download_dir: PathBuf,
    ) -> Self {
        DownloadImgTask {
            app: download_task.app.clone(),
            download_task,
            url,
            index,
            temp_download_dir,
        }
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            index = self.index,
            url = &self.url,
            comic_uuid = self.download_task.comic.comic.uuid,
            comic_title = self.download_task.comic.comic.name,
            group_name = self.download_task.chapter_info.group_name,
            group_path_word = self.download_task.chapter_info.group_path_word,
            chapter_uuid = self.download_task.chapter_info.chapter_uuid,
            order = self.download_task.chapter_info.order
        )
    )]
    pub async fn process(self) {
        let download_img_task = self.download_img();
        tokio::pin!(download_img_task);

        let mut state_receiver = self.download_task.state_sender.subscribe();
        state_receiver.mark_changed();

        let mut delete_receiver = self.download_task.delete_sender.subscribe();

        let mut permit = None;

        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            tokio::select! {
                () = &mut download_img_task, if state_is_downloading && permit.is_some() => break,

                () = self.acquire_img_permit(&mut permit), if state_is_downloading && permit.is_none() => {},

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

    #[instrument(level = "error", skip_all)]
    async fn download_img(&self) {
        let url = &self.url;

        let download_format = self.app.get_config().read().download_format;
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

            tracing::trace!("图片已存在，跳过下载");
            return;
        }

        tracing::trace!("开始下载图片");

        let copy_client = self.app.get_copy_client();
        let (img_data, img_format) = match copy_client.get_img_data_and_format(url).await {
            Ok(data_and_format) => data_and_format,
            Err(err) => {
                let err_title = "下载图片失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                return;
            }
        };
        let img_data_len = img_data.len() as u64;

        tracing::trace!("图片成功下载到内存");

        // 保存图片
        let target_format = download_format.to_image_format();
        if let Err(err) = save_img(&save_path, target_format, &img_data, img_format) {
            let err_title = "保存图片失败";
            let message = err.to_message();
            tracing::error!(err_title, message);
            return;
        }

        // 记录下载字节数
        self.app
            .get_download_manager()
            .byte_per_sec
            .fetch_add(img_data_len, Ordering::Relaxed);

        self.download_task
            .downloaded_img_count
            .fetch_add(1, Ordering::Relaxed);

        self.download_task.emit_download_task_update_event();

        let img_download_interval_sec = self.app.get_config().read().img_download_interval_sec;
        sleep(Duration::from_secs(img_download_interval_sec)).await;
    }

    #[instrument(level = "error", skip_all)]
    async fn acquire_img_permit<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        tracing::trace!("图片开始排队");

        *permit = match permit.take() {
            // 如果有permit，则直接用
            Some(permit) => Some(permit),
            // 如果没有permit，则获取permit
            None => match self
                .app
                .get_download_manager()
                .inner()
                .img_sem
                .acquire()
                .await
                .map_err(eyre::Report::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title = "获取下载图片的permit失败";
                    let message = err.to_message();
                    tracing::error!(err_title, message);
                    return;
                }
            },
        };
    }

    #[instrument(level = "error", skip_all)]
    async fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) {
        let state = *state_receiver.borrow();
        if state == DownloadTaskState::Paused {
            // 稍微等一下再释放permit
            // 避免大批量暂停时，本应暂停的任务因拿到permit而稍微下载一小段(虽然最终会被暂停)
            sleep(Duration::from_millis(100)).await;
            tracing::trace!("图片暂停下载");
            if let Some(permit) = permit.take() {
                drop(permit);
            }
        } else if state == DownloadTaskState::Failed {
            // 稍微等一下再释放permit
            // 避免大批量失败时，本应失败的任务因拿到permit而稍微下载一小段(虽然最终会被失败)
            sleep(Duration::from_millis(100)).await;
            tracing::trace!("图片取消下载");
            if let Some(permit) = permit.take() {
                drop(permit);
            }
        }
    }

    #[instrument(level = "error", skip_all)]
    async fn handle_delete_receiver_change<'a>(&'a self, permit: &mut Option<SemaphorePermit<'a>>) {
        if permit.is_some() {
            // 如果有permit则稍微等一下再退出
            // 这是为了避免大批量删除时，本应删除的任务因拿到permit而又稍微下载一小段
            sleep(Duration::from_millis(100)).await;
        }

        tracing::trace!("图片取消下载");
    }
}

#[instrument(level = "error", skip_all, fields(save_path = ?save_path, target_format = ?target_format, src_format = ?src_format))]
fn save_img(
    save_path: &Path,
    target_format: ImageFormat,
    src_img_data: &Bytes,
    src_format: ImageFormat,
) -> eyre::Result<()> {
    if target_format == src_format {
        // 如果target_format与src_format匹配，则直接保存
        std::fs::write(save_path, src_img_data)?;
        tracing::trace!("图片成功保存到磁盘");
        return Ok(());
    }
    // 如果target_format与src_format不匹配，则需要转换格式
    let img = image::load_from_memory(src_img_data)?;

    let mut converted_data = Vec::new();
    match target_format {
        ImageFormat::WebP => img
            .to_rgba8()
            .write_to(&mut Cursor::new(&mut converted_data), ImageFormat::WebP),
        ImageFormat::Jpeg => img
            .to_rgb8()
            .write_to(&mut Cursor::new(&mut converted_data), ImageFormat::Jpeg),
        _ => return Err(eyre!("不支持的图片格式: {:?}", target_format)),
    }
    .wrap_err("转换图片格式失败")?;

    std::fs::write(save_path, &converted_data)?;
    tracing::trace!("图片成功保存到磁盘");

    Ok(())
}
