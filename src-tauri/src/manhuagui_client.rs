use std::time::Duration;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::{
    config::Config,
    decrypt::decrypt,
    extensions::SendWithTimeoutMsg,
    types::{ChapterInfo, Comic, GetFavoriteResult, SearchResult, UserProfile},
};

#[derive(Clone)]
pub struct ManhuaguiClient {
    app: AppHandle,
    api_client: ClientWithMiddleware,
    img_client: ClientWithMiddleware,
}

impl ManhuaguiClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client();
        let img_client = create_img_client();

        Self {
            app,
            api_client,
            img_client,
        }
    }

    pub async fn login(&self, username: &str, password: &str) -> anyhow::Result<String> {
        let params = json!({"action": "user_login"});
        let form = json!({
            "txtUserName": username,
            "txtPassword": password,
        });
        // 发送登录请求
        let http_resp = self
            .api_client
            .get("https://www.manhuagui.com/tools/submit_ajax.ashx")
            .query(&params)
            .form(&form)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let headers = http_resp.headers().clone();
        let body = http_resp.text().await?;
        if status == StatusCode::FOUND {
            return Err(anyhow!("cookie已过期或无效"));
        } else if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }
        // 获取resp header中的set-cookie字段
        let cookie = headers
            .get("set-cookie")
            .ok_or(anyhow!("响应中没有set-cookie字段: {body}"))?
            .to_str()
            .context(format!("响应中的set-cookie字段不是utf-8字符串: {body}"))?
            .to_string();

        Ok(cookie)
    }

    pub async fn get_user_profile(&self) -> anyhow::Result<UserProfile> {
        let cookie = self.app.state::<RwLock<Config>>().read().cookie.clone();
        // 发送获取用户信息请求
        let http_resp = self
            .api_client
            .get("https://www.manhuagui.com/user/center/index")
            .header("cookie", cookie)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == StatusCode::FOUND {
            return Err(anyhow!("未登录、cookie已过期或cookie无效"));
        } else if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }

        let user_profile = UserProfile::from_html(&body).context("将body转换为UserProfile失败")?;
        Ok(user_profile)
    }

    pub async fn search(&self, keyword: &str, page_num: i64) -> anyhow::Result<SearchResult> {
        let url = format!("https://www.manhuagui.com/s/{keyword}_p{page_num}.html");
        let http_resp = self.api_client.get(url).send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }
        let search_result =
            SearchResult::from_html(&body).context("将body转换为SearchResult失败")?;
        Ok(search_result)
    }

    pub async fn get_comic(&self, id: i64) -> anyhow::Result<Comic> {
        let http_resp = self
            .api_client
            .get(format!("https://www.manhuagui.com/comic/{id}/"))
            .send_with_timeout_msg()
            .await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }
        let comic = Comic::from_html(&self.app, &body).context("将body转换为Comic失败")?;

        Ok(comic)
    }

    pub async fn get_img_urls(&self, chapter_info: &ChapterInfo) -> anyhow::Result<Vec<String>> {
        let comic_id = chapter_info.comic_id;
        let chapter_id = chapter_info.chapter_id;

        let url = format!("https://www.manhuagui.com/comic/{comic_id}/{chapter_id}.html");
        let http_resp = self.api_client.get(url).send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }

        let decrypt_result = decrypt(&body).context("解密失败")?;

        let urls = decrypt_result
            .files
            .iter()
            .map(|file| format!("https://i.hamreus.com{}{file}", decrypt_result.path))
            .map(|url| url.trim_end_matches(".webp").to_string())
            .collect();

        Ok(urls)
    }

    pub async fn get_img_bytes(&self, url: &str) -> anyhow::Result<Bytes> {
        // 发送下载图片请求
        let http_resp = self
            .img_client
            .get(url)
            .header("referer", "https://www.manhuagui.com/")
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }
        // 读取图片数据
        let image_data = http_resp.bytes().await?;

        Ok(image_data)
    }

    pub async fn get_favorite(&self, page_num: i64) -> anyhow::Result<GetFavoriteResult> {
        let cookie = self.app.state::<RwLock<Config>>().read().cookie.clone();
        // 发送获取收藏夹请求
        let url = format!("https://www.manhuagui.com/user/book/shelf/{page_num}");
        let http_resp = self
            .api_client
            .get(url)
            .header("cookie", cookie)
            .send_with_timeout_msg()
            .await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }
        // 解析html
        let get_favorite_result =
            GetFavoriteResult::from_html(&body).context("将body转换为GetFavoriteResult失败")?;
        Ok(get_favorite_result)
    }
}

fn create_api_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1) // 指数为1，保证重试间隔为1秒不变
        .jitter(Jitter::Bounded) // 重试间隔在1秒左右波动
        .build_with_total_retry_duration(Duration::from_secs(5)); // 重试总时长为5秒

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(3)) // 每个请求超过3秒就超时
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_img_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client = reqwest::ClientBuilder::new().build().unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
