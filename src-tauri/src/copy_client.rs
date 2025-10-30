use std::{io::Cursor, sync::Arc, time::Duration};

use anyhow::{anyhow, Context};
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use image::ImageFormat;
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::task::JoinSet;

use crate::{
    account_pool::{Account, AccountPool},
    config::Config,
    errors::{CopyMangaError, CopyMangaResult, RiskControlError},
    extensions::SendWithTimeoutMsg,
    responses::{
        ChapterInGetChaptersRespData, CopyResp, GetChapterRespData, GetChaptersRespData,
        GetComicRespData, GetFavoriteRespData, LoginRespData, SearchRespData, UserProfileRespData,
    },
    types::{AsyncRwLock, DownloadFormat},
};

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

    pub async fn register(&self, username: &str, password: &str) -> CopyMangaResult<()> {
        // 发送注册请求
        let form = json!({
            "username": username,
            "password": password,
            "source": "freeSite"
        });
        let api_domain = self.get_api_domain();
        let http_resp = self
            .api_client
            .post(format!("https://{api_domain}/api/v3/register"))
            .form(&form)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::Register(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("注册失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("注册失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("注册失败，预料之外的code: {copy_resp:?}").into());
        }

        Ok(())
    }

    pub async fn login(&self, username: &str, password: &str) -> CopyMangaResult<LoginRespData> {
        // 对密码进行编码
        const SALT: i32 = 1729;
        let password = general_purpose::STANDARD.encode(format!("{password}-{SALT}").as_bytes());
        let form = json!( {
            "username": username,
            "password": password,
            "salt": SALT,
        });
        // 发送登录请求
        let api_domain = self.get_api_domain();
        let http_resp = self
            .api_client
            .post(format!("https://{api_domain}/api/v3/login"))
            .form(&form)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::Login(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("使用账号密码登录失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body).context(format!(
            "使用账号密码登录失败，将body解析为CopyResp失败: {body}"
        ))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(
                anyhow!("使用账号密码登录失败，CopyResp的code字段不为200: {copy_resp:?}").into(),
            );
        }
        // 尝试将CopyResp的results字段解析为LoginRespData
        let results_str = copy_resp.results.to_string();
        let login_resp_data = serde_json::from_str::<LoginRespData>(&results_str).context(
            format!("使用账号密码登录失败，将results解析为LoginRespData失败: {results_str}"),
        )?;

        Ok(login_resp_data)
    }

    pub async fn get_user_profile(&self) -> CopyMangaResult<UserProfileRespData> {
        // 发送获取用户信息请求
        let api_domain = self.get_api_domain();
        let http_resp = self
            .api_client
            .get(format!("https://{api_domain}/api/v3/member/info"))
            .header("authorization", self.get_authorization())
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::GetUserProfile(body).into());
        } else if status == 401 {
            return Err(anyhow!("获取用户信息失败，token错误或过期: {body}").into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("获取用户信息失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body).context(format!(
            "获取用户信息失败，将body解析为CopyResp失败: {body}"
        ))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取用户信息失败，预料之外的code: {copy_resp:?}").into());
        }
        // 尝试将CopyResp的results字段解析为UserProfileRespData
        let results_str = copy_resp.results.to_string();
        let user_profile_resp_data = serde_json::from_str::<UserProfileRespData>(&results_str)
            .context(format!(
                "获取用户信息失败，将results解析为UserProfileRespData失败: {results_str}"
            ))?;

        Ok(user_profile_resp_data)
    }

    pub async fn search(&self, keyword: &str, page_num: i64) -> CopyMangaResult<SearchRespData> {
        const LIMIT: i64 = 20;
        // page_num从1开始
        let offset = (page_num - 1) * LIMIT;
        let params = json!({
            "limit": LIMIT,
            "offset": offset,
            "q": keyword,
            "q_type": "",
            "platform": 1,
        });
        // 发送搜索请求
        let api_domain = self.get_api_domain();
        let http_resp = self
            .api_client
            .get(format!("https://{api_domain}/api/v3/search/comic"))
            .query(&params)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::Search(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("搜索漫画失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("搜索漫画失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("搜索漫画失败，预料之外的code: {copy_resp:?}").into());
        }
        // 尝试将CopyResp的results字段解析为SearchRespData
        let results_str = copy_resp.results.to_string();
        let search_resp_data = serde_json::from_str::<SearchRespData>(&results_str).context(
            format!("搜索漫画失败，将results解析为SearchRespData失败: {results_str}"),
        )?;

        Ok(search_resp_data)
    }

    pub async fn get_comic(&self, comic_path_word: &str) -> CopyMangaResult<GetComicRespData> {
        let params = json!({
            "platform": 1,
        });
        // 发送获取漫画请求
        let api_domain = self.get_api_domain();
        let url = format!("https://{api_domain}/api/v3/comic2/{comic_path_word}");
        let http_resp = self
            .api_client
            .get(url)
            .query(&params)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::GetComic(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("获取漫画失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("获取漫画失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取漫画失败，预料之外的code: {copy_resp:?}").into());
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
    ) -> CopyMangaResult<Vec<ChapterInGetChaptersRespData>> {
        const LIMIT: i64 = 100;
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
                Ok::<_, CopyMangaError>(chapter_resp_data)
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
    ) -> CopyMangaResult<GetChaptersRespData> {
        let params = json!({
            "limit": limit,
            "offset": offset,
        });
        // 发送获取章节分页请求
        let api_domain = self.get_api_domain();
        let url = format!(
            "https://{api_domain}/api/v3/comic/{comic_path_word}/group/{group_path_word}/chapters"
        );
        let http_resp = self
            .api_client
            .get(url)
            .query(&params)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::GetChapters(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("获取章节分页失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body).context(format!(
            "获取章节分页失败，将body解析为CopyResp失败: {body}"
        ))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取章节分页失败，预料之外的code: {copy_resp:?}").into());
        }
        // 尝试将CopyResp的results字段解析为GetChaptersRespData
        let results_str = copy_resp.results.to_string();
        let get_chapters_resp_data = serde_json::from_str::<GetChaptersRespData>(&results_str)
            .context(format!(
                "获取章节分页失败，将results解析为GetChaptersRespData失败: {results_str}"
            ))?;

        Ok(get_chapters_resp_data)
    }

    pub async fn get_chapter(
        &self,
        comic_path_word: &str,
        chapter_uuid: &str,
    ) -> CopyMangaResult<GetChapterRespData> {
        let account = if let Some(account) = self.get_account_from_pool().await {
            account
        } else {
            // 如果账号池里没有合适的账号
            let account_pool = self.app.state::<AsyncRwLock<AccountPool>>();
            let mut account_pool = account_pool.write().await;
            // 拿到写锁后再次检查账号池里是否有合适的账号
            // 如果有，就用账号池里的账号，否则才注册一个新账号
            // 确认一下是因为可能在拿到写锁之前，其他线程已经注册了一个新账号，避免重复注册
            match account_pool.get_available_account() {
                Some(account) => account,
                None => account_pool.register().await?,
            }
        };

        let token = account.read().token.clone();
        let authorization = format!("Token {token}");

        let params = json!({
            "platform": 1,
        });
        // 发送获取章节请求
        let api_domain = self.get_api_domain();
        let url =
            format!("https://{api_domain}/api/v3/comic/{comic_path_word}/chapter2/{chapter_uuid}");
        let resp = self
            .api_client
            .get(url)
            .query(&params)
            .header("authorization", authorization)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = resp.status();
        let body = resp.text().await?;
        if status == 210 {
            // 如果当前账号被风控，就更新账号的limited_at字段
            account.write().limited_at = chrono::Local::now().timestamp();
            self.app
                .state::<AsyncRwLock<AccountPool>>()
                .write()
                .await
                .save()?;
            return Err(RiskControlError::GetChapter(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("获取章节失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("获取章节失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取章节失败，预料之外的code: {copy_resp:?}").into());
        }
        // 尝试将CopyResp的results字段解析为GetChapterRespData
        let results_str = copy_resp.results.to_string();
        let get_chapter_resp_data = serde_json::from_str::<GetChapterRespData>(&results_str)
            .context(format!(
                "获取章节失败，将results解析为GetChapterRespData失败: {results_str}"
            ))?;

        Ok(get_chapter_resp_data)
    }

    pub async fn get_img_data_and_format(&self, url: &str) -> anyhow::Result<(Bytes, ImageFormat)> {
        // 发送下载图片请求
        let http_resp = self.img_client.get(url).send_with_timeout_msg().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(anyhow!(
                "下载图片 {url} 失败，预料之外的状态码({status}): {body}"
            ));
        }
        // 获取 resp headers 的 content-type 字段
        let content_type = http_resp
            .headers()
            .get("content-type")
            .ok_or(anyhow!("响应中没有content-type字段"))?
            .to_str()
            .context("响应中的content-type字段不是utf-8字符串")?
            .to_string();
        // 读取图片数据
        let image_data = http_resp.bytes().await?;
        let original_format = match content_type.as_str() {
            "image/webp" => ImageFormat::WebP,
            "image/jpeg" => ImageFormat::Jpeg,
            _ => return Err(anyhow!("原图出现了意料之外的格式: {content_type}")),
        };
        // 确定目标格式
        let download_format = self.app.state::<RwLock<Config>>().read().download_format;
        let target_format = match download_format {
            DownloadFormat::Webp => ImageFormat::WebP,
            DownloadFormat::Jpeg => ImageFormat::Jpeg,
        };
        // 如果原始格式与目标格式相同，直接返回
        if original_format == target_format {
            return Ok((image_data, original_format));
        }
        // 否则需要将图片转换为目标格式
        let img =
            image::load_from_memory(&image_data).context("将图片数据转换为DynamicImage失败")?;
        let mut converted_data = Vec::new();
        match target_format {
            ImageFormat::Jpeg => img
                .to_rgb8()
                .write_to(&mut Cursor::new(&mut converted_data), target_format),
            ImageFormat::WebP => img
                .to_rgba8()
                .write_to(&mut Cursor::new(&mut converted_data), target_format),
            _ => return Err(anyhow!("这里不应该出现目标格式`{target_format:?}`")),
        }
        .context(format!(
            "将`{original_format:?}`转换为`{target_format:?}`失败"
        ))?;

        Ok((Bytes::from(converted_data), target_format))
    }

    pub async fn get_favorite(&self, page_num: i64) -> CopyMangaResult<GetFavoriteRespData> {
        const LIMIT: i64 = 18;
        let params = json!({
            "limit": LIMIT,
            "offset": (page_num - 1) * LIMIT,
            "free_type": 1,
            "ordering": "-datetime_modifier",
        });
        // 发送获取收藏请求
        let api_domain = self.get_api_domain();
        let http_resp = self
            .api_client
            .get(format!("https://{api_domain}/api/v3/member/collect/comics"))
            .query(&params)
            .header("authorization", self.get_authorization())
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == 210 {
            return Err(RiskControlError::GetFavorite(body).into());
        } else if status != StatusCode::OK {
            return Err(anyhow!("获取收藏失败，预料之外的状态码({status}): {body}").into());
        }
        // 尝试将body解析为CopyResp
        let copy_resp = serde_json::from_str::<CopyResp>(&body)
            .context(format!("获取收藏失败，将body解析为CopyResp失败: {body}"))?;
        // 检查CopyResp的code字段
        if copy_resp.code != 200 {
            return Err(anyhow!("获取收藏失败，预料之外的code: {copy_resp:?}").into());
        }
        // 尝试将CopyResp的results字段解析为GetFavoriteRespData
        let results_str = copy_resp.results.to_string();
        let get_favorite_resp_data = serde_json::from_str::<GetFavoriteRespData>(&results_str)
            .context(format!(
                "获取收藏失败，将results解析为GetFavoriteRespData失败: {results_str}"
            ))?;

        Ok(get_favorite_resp_data)
    }

    fn get_authorization(&self) -> String {
        self.app
            .state::<RwLock<Config>>()
            .read()
            .get_authorization()
    }

    fn get_api_domain(&self) -> String {
        self.app.state::<RwLock<Config>>().read().get_api_domain()
    }

    async fn get_account_from_pool(&self) -> Option<Arc<RwLock<Account>>> {
        self.app
            .state::<AsyncRwLock<AccountPool>>()
            .read()
            .await
            .get_available_account()
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
    use reqwest::header::{HeaderMap, HeaderValue};

    let retry_policy = ExponentialBackoff::builder()
        .base(1) // 指数为1，保证重试间隔为1秒不变
        .jitter(Jitter::Bounded) // 重试间隔在1秒左右波动
        .build_with_total_retry_duration(Duration::from_secs(5)); // 重试总时长为5秒

    let from_static = HeaderValue::from_static;

    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", from_static("COPY/3.0.0"));
    headers.insert("Accept", from_static("application/json"));
    headers.insert("version", from_static("2025.08.15"));
    headers.insert("platform", from_static("1"));
    headers.insert("webp", from_static("1"));
    headers.insert("region", from_static("1"));

    let client = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .timeout(Duration::from_secs(3)) // 每个请求超过3秒就超时
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
