use std::time::Duration;

use anyhow::{anyhow, Context};
use base64::{engine::general_purpose, Engine};
use reqwest::{Client, ClientBuilder, StatusCode};
use serde_json::json;
use tauri::AppHandle;

use crate::responses::{CopyResp, LoginRespData};

const API_DOMAIN: &str = "api.mangacopy.com";

#[derive(Clone)]
pub struct CopyClient {
    app: AppHandle,
}

impl CopyClient {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn client(&self) -> Client {
        // TODO: 添加重试机制
        ClientBuilder::new()
            .timeout(Duration::from_secs(2)) // 每个请求超过2秒就超时
            .build()
            .unwrap()
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
        let http_resp = self
            .client()
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
}
