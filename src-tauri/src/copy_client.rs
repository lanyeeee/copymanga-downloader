use std::time::Duration;

use anyhow::{anyhow, Context};
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::task::JoinSet;

use crate::{
    config::Config,
    responses::{
        ChapterInGetChaptersRespData, CopyResp, GetChapterRespData, GetChaptersRespData,
        GetComicRespData, LoginRespData, SearchRespData, UserProfileRespData,
    },
};

const API_DOMAIN: &str = "api.mangacopy.com";

#[derive(Clone)]
pub struct CopyClient {
    app: AppHandle,
    api_client: ClientWithMiddleware,
    img_client: ClientWithMiddleware,
}

impl CopyClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client();
        let img_client = create_img_client();
        Self {
            app,
            api_client,
            img_client,
        }
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
            .api_client
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
        let http_resp = self
            .api_client
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
        let http_resp = self.api_client
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

    pub async fn get_comic(&self, comic_path_word: &str) -> anyhow::Result<GetComicRespData> {
        let authorization = self.get_authorization();
        // 发送获取漫画请求
        let http_resp = self.api_client
            .get(format!("https://{API_DOMAIN}/api/v3/comic2/{comic_path_word}"))
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
        let get_comic_resp_data = serde_json::from_str::<GetComicRespData>(&results_str).context(
            format!("获取漫画失败，将results解析为ComicRespData失败: {results_str}"),
        )?;

        Ok(get_comic_resp_data)
    }

    pub async fn get_group_chapters(
        &self,
        comic_path_word: &str,
        group_path_word: &str,
    ) -> anyhow::Result<Vec<ChapterInGetChaptersRespData>> {
        const LIMIT: i64 = 500;
        let mut chapters = vec![];
        // 获取第一页的章节
        let mut first_chapters_resp_data = self
            .get_chapters(comic_path_word, group_path_word, LIMIT, 0)
            .await?;
        // 将第一页的章节添加到chapters中
        chapters.append(&mut first_chapters_resp_data.list);
        // 计算总页数
        let total_pages = first_chapters_resp_data.total / LIMIT + 1;
        // 如果只有一页，直接返回
        if total_pages == 1 {
            return Ok(chapters);
        }
        // 并发获取剩余页的章节
        let mut join_set = JoinSet::new();
        for page in 2..=total_pages {
            let comic_path_word = comic_path_word.to_string();
            let group_path_word = group_path_word.to_string();
            let copy_client = self.clone();
            join_set.spawn(async move {
                let offset = (page - 1) * LIMIT;
                let chapter_resp_data = copy_client
                    .get_chapters(&comic_path_word, &group_path_word, LIMIT, offset)
                    .await?;
                Ok::<_, anyhow::Error>(chapter_resp_data)
            });
        }
        // 将剩余页的章节添加到chapters中
        while let Some(res) = join_set.join_next().await {
            let mut chapter_resp_data = res??;
            chapters.append(&mut chapter_resp_data.list);
        }

        Ok(chapters)
    }

    pub async fn get_chapters(
        &self,
        comic_path_word: &str,
        group_path_word: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<GetChaptersRespData> {
        let params = json!({
            "limit": limit,
            "offset": offset,
        });
        let authorization = self.get_authorization();
        // TODO: 错误提示改成 获取章节分页
        // 发送获取章节请求
        let http_resp = self.api_client
            .get(format!("https://{API_DOMAIN}/api/v3/comic/{comic_path_word}/group/{group_path_word}/chapters"))
            .query(&params)
            .header("authorization", authorization)
            .header("version", "2.2.0")
            .header("platform", "3")
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .send()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取章节失败，预料之外的状态码({status}): {body}"));
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("获取章节失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取章节失败，预料之外的code: {copy_resp:?}"));
        }
        // 尝试将CopyResp的results字段解析为ChapterRespData
        let results_str = copy_resp.results.to_string();
        let get_chapters_resp_data = serde_json::from_str::<GetChaptersRespData>(&results_str)
            .context(format!(
                "获取章节失败，将results解析为ChapterRespData失败: {results_str}"
            ))?;

        Ok(get_chapters_resp_data)
    }

    pub async fn get_chapter(
        &self,
        comic_path_word: &str,
        chapter_uuid: &str,
    ) -> anyhow::Result<GetChapterRespData> {
        let authorization = self.get_authorization();
        // 发送获取章节请求
        let resp = self.api_client
            .get(format!("https://{API_DOMAIN}/api/v3/comic/{comic_path_word}/chapter2/{chapter_uuid}"))
            .header("authorization", authorization)
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .send()
            .await?;
        // 检查http响应状态码
        let status = resp.status();
        let body = resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取章节失败，预料之外的状态码({status}): {body}"));
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("获取章节失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取章节失败，预料之外的code: {copy_resp:?}"));
        }
        // 尝试将CopyResp的results字段解析为ChapterRespData
        let results_str = copy_resp.results.to_string();
        let get_chapter_resp_data = serde_json::from_str::<GetChapterRespData>(&results_str)
            .context(format!(
                "获取章节失败，将results解析为ChapterRespData失败: {results_str}"
            ))?;

        Ok(get_chapter_resp_data)
    }

    pub async fn get_image_bytes(&self, url: &str) -> anyhow::Result<Bytes> {
        // 发送下载图片请求
        let http_resp = self.img_client.get(url).send().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(anyhow!(
                "下载图片 {url} 失败，预料之外的状态码({status}): {body}"
            ));
        }
        // 读取图片数据
        let image_data = http_resp.bytes().await?;

        Ok(image_data)
    }

    fn get_authorization(&self) -> String {
        self.app
            .state::<RwLock<Config>>()
            .read()
            .get_authorization()
    }
}

fn create_img_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client = reqwest::ClientBuilder::new().build().unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_api_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1) // 指数为1，保证重试间隔为1秒不变
        .jitter(Jitter::Bounded) // 重试间隔在1秒左右波动
        .build_with_total_retry_duration(Duration::from_secs(5)); // 重试总时长为5秒

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(3)) // 每个请求超过3秒就超时
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
