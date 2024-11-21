use std::time::Duration;

use anyhow::{anyhow, Context};
use base64::{engine::general_purpose, Engine};
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::{
    config::Config,
    responses::{ComicRespData, CopyResp, LoginRespData, SearchRespData, UserProfileRespData},
};

const API_DOMAIN: &str = "api.mangacopy.com";

#[derive(Clone)]
pub struct CopyClient {
    app: AppHandle,
}

impl CopyClient {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn client() -> ClientWithMiddleware {
        let retry_policy = ExponentialBackoff::builder()
            .base(1) // 指数为1，保证重试间隔为1秒不变
            .jitter(Jitter::Bounded) // 重试间隔在1秒左右波动
            .build_with_total_retry_duration(Duration::from_secs(3)); // 重试总时长为3秒
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(2)) // 每个请求超过2秒就超时
            .build()
            .unwrap();
        reqwest_middleware::ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    }

    pub async fn login(&self, username: &str, password: &str) -> anyhow::Result<LoginRespData> {
        // 对密码进行编码
        const SALT: i32 = 1729;
        let password = general_purpose::STANDARD.encode(format!("{password}-{SALT}").as_bytes());
        let form = json!( {
            "username": username,
            "password": password,
            "salt": SALT,
        });
        // 发送登录请求
        let http_resp = Self::client()
            .post(format!("https://{API_DOMAIN}/api/v3/login"))
            .form(&form)
            .send()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!(
                "使用账号密码登录失败，预料之外的状态码({status}): {body}"
            ));
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body).context(format!(
            "使用账号密码登录失败，将body解析为CopyResp失败: {body}"
        ))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!(
                "使用账号密码登录失败，CopyResp的code字段不为200: {copy_resp:?}"
            ));
        }
        // 尝试将CopyResp的results字段解析为LoginRespData
        let results_str = copy_resp.results.to_string();
        let login_resp_data = serde_json::from_str::<LoginRespData>(&results_str).context(
            format!("使用账号密码登录失败，将results解析为LoginRespData失败: {results_str}"),
        )?;

        Ok(login_resp_data)
    }

    pub async fn get_user_profile(&self) -> anyhow::Result<UserProfileRespData> {
        let authorization = self.get_authorization();
        // 发送获取用户信息请求
        let http_resp = Self::client()
            .get(format!("https://{API_DOMAIN}/api/v3/member/info"))
            .header("authorization", authorization)
            .send()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        // TODO: 处理401状态码，token错误或过期
        if status != StatusCode::OK {
            return Err(anyhow!(
                "获取用户信息失败，预料之外的状态码({status}): {body}"
            ));
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body).context(format!(
            "获取用户信息失败，将body解析为CopyResp失败: {body}"
        ))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取用户信息失败，预料之外的code: {copy_resp:?}"));
        }
        // 尝试将CopyResp的results字段解析为UserProfileRespData
        let results_str = copy_resp.results.to_string();
        let user_profile_resp_data = serde_json::from_str::<UserProfileRespData>(&results_str)
            .context(format!(
                "获取用户信息失败，将results解析为UserProfileRespData失败: {results_str}"
            ))?;

        Ok(user_profile_resp_data)
    }

    pub async fn search(&self, keyword: &str, page_num: i64) -> anyhow::Result<SearchRespData> {
        const LIMIT: i64 = 20;
        // page_num从1开始
        let offset = (page_num - 1) * LIMIT;
        let params = json!({
            "limit": LIMIT,
            "offset": offset,
            "q": keyword,
        });
        // 发送搜索请求
        let http_resp = Self::client()
            .get(format!("https://{API_DOMAIN}/api/v3/search/comic"))
            .query(&params)
            .header("version", "2.2.0")
            .header("platform", "3")
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .send()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("搜索漫画失败，预料之外的状态码({status}): {body}"));
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("搜索漫画失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("搜索漫画失败，预料之外的code: {copy_resp:?}"));
        }
        // 尝试将CopyResp的results字段解析为SearchRespData
        let results_str = copy_resp.results.to_string();
        let search_resp_data = serde_json::from_str::<SearchRespData>(&results_str).context(
            format!("搜索漫画失败，将results解析为SearchRespData失败: {results_str}"),
        )?;

        Ok(search_resp_data)
    }

    pub async fn get_comic(&self, path_word: String) -> anyhow::Result<ComicRespData> {
        let authorization = self.get_authorization();
        // 发送获取漫画请求
        let http_resp = Self::client()
            .get(format!("https://{API_DOMAIN}/api/v3/comic2/{path_word}"))
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .header("authorization", authorization)
            .send()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取漫画失败，预料之外的状态码({status}): {body}"));
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("获取漫画失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取漫画失败，预料之外的code: {copy_resp:?}"));
        }
        // 尝试将CopyResp的results字段解析为ComicRespData
        let results_str = copy_resp.results.to_string();
        let comic_resp_data = serde_json::from_str::<ComicRespData>(&results_str).context(
            format!("获取漫画失败，将results解析为ComicRespData失败: {results_str}"),
        )?;

        Ok(comic_resp_data)
    }

    fn get_authorization(&self) -> String {
        self.app
            .state::<RwLock<Config>>()
            .read()
            .get_authorization()
    }
}
