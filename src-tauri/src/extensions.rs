use parking_lot::RwLock;
use reqwest::Response;
use reqwest_middleware::RequestBuilder;
use tauri::{AppHandle, Manager, State};

use crate::{
    account_pool::AccountPool, config::Config, copy_client::CopyClient,
    download_manager::DownloadManager, types::AsyncRwLock,
};

pub trait AnyhowErrorToStringChain {
    /// 将 `anyhow::Error` 转换为chain格式
    /// # Example
    /// 0: error message
    /// 1: error message
    /// 2: error message
    fn to_string_chain(&self) -> String;
}

impl AnyhowErrorToStringChain for anyhow::Error {
    fn to_string_chain(&self) -> String {
        use std::fmt::Write;
        self.chain()
            .enumerate()
            .fold(String::new(), |mut output, (i, e)| {
                let _ = writeln!(output, "{i}: {e}");
                output
            })
    }
}

pub trait SendWithTimeoutMsg {
    /// 发送请求并处理超时错误
    ///
    /// - 如果遇到超时错误，返回带有用户友好信息的错误
    /// - 否则返回原始错误
    async fn send_with_timeout_msg(self) -> anyhow::Result<Response>;
}

impl SendWithTimeoutMsg for RequestBuilder {
    async fn send_with_timeout_msg(self) -> anyhow::Result<Response> {
        self.send().await.map_err(|e| {
            if e.is_timeout() || e.is_middleware() {
                anyhow::Error::from(e).context("网络连接超时，请使用代理或换条线路重试")
            } else {
                anyhow::Error::from(e)
            }
        })
    }
}

pub trait WalkDirEntryExt {
    fn is_comic_metadata(&self) -> bool;
    fn is_chapter_metadata(&self) -> bool;
}
impl WalkDirEntryExt for walkdir::DirEntry {
    fn is_comic_metadata(&self) -> bool {
        if !self.file_type().is_file() {
            return false;
        }
        if self.file_name() != "元数据.json" {
            return false;
        }

        true
    }

    fn is_chapter_metadata(&self) -> bool {
        if !self.file_type().is_file() {
            return false;
        }
        if self.file_name() != "章节元数据.json" {
            return false;
        }

        true
    }
}

pub trait PathIsImg {
    /// 判断路径是否为图片文件
    fn is_img(&self) -> bool;
}

impl PathIsImg for std::path::Path {
    fn is_img(&self) -> bool {
        self.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase)
            .is_some_and(|ext| matches!(ext.as_str(), "jpg" | "webp"))
    }
}

pub trait AppHandleExt {
    fn get_config(&self) -> State<RwLock<Config>>;
    fn get_copy_client(&self) -> State<CopyClient>;
    fn get_download_manager(&self) -> State<DownloadManager>;
    fn get_account_pool(&self) -> State<AsyncRwLock<AccountPool>>;
}

impl AppHandleExt for AppHandle {
    fn get_config(&self) -> State<RwLock<Config>> {
        self.state::<RwLock<Config>>()
    }
    fn get_copy_client(&self) -> State<CopyClient> {
        self.state::<CopyClient>()
    }
    fn get_download_manager(&self) -> State<DownloadManager> {
        self.state::<DownloadManager>()
    }
    fn get_account_pool(&self) -> State<AsyncRwLock<AccountPool>> {
        self.state::<AsyncRwLock<AccountPool>>()
    }
}
