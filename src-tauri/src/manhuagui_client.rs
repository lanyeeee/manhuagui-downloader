use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use eyre::{eyre, OptionExt, WrapErr};
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde_json::json;
use tauri::AppHandle;

use crate::{
    config::ProxyMode,
    decrypt::decrypt,
    extensions::{AppHandleExt, ReportToStringChain, SendWithTimeoutMsg},
    types::{ChapterInfo, Comic, GetFavoriteResult, SearchResult, UserProfile},
};

#[derive(Clone)]
pub struct ManhuaguiClient {
    app: AppHandle,
    api_client: Arc<RwLock<ClientWithMiddleware>>,
    img_client: Arc<RwLock<ClientWithMiddleware>>,
}

impl ManhuaguiClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client(&app);
        let api_client = Arc::new(RwLock::new(api_client));

        let img_client = create_img_client(&app);
        let img_client = Arc::new(RwLock::new(img_client));

        Self {
            app,
            api_client,
            img_client,
        }
    }

    pub fn reload_client(&self) {
        let api_client = create_api_client(&self.app);
        *self.api_client.write() = api_client;
        let img_client = create_img_client(&self.app);
        *self.img_client.write() = img_client;
    }

    pub async fn login(&self, username: &str, password: &str) -> eyre::Result<String> {
        let params = json!({"action": "user_login"});
        let form = json!({
            "txtUserName": username,
            "txtPassword": password,
        });
        // 发送登录请求
        let request = self
            .api_client
            .read()
            .get("https://www.manhuagui.com/tools/submit_ajax.ashx")
            .query(&params)
            .form(&form);
        let http_resp = request.send_with_timeout_msg().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let headers = http_resp.headers().clone();
        let body = http_resp.text().await?;
        if status == StatusCode::FOUND {
            return Err(eyre!("cookie已过期或无效"));
        } else if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        // 获取resp header中的set-cookie字段
        let cookie = headers
            .get("set-cookie")
            .ok_or_eyre(format!("响应中没有set-cookie字段: {body}"))?
            .to_str()
            .wrap_err(format!("响应中的set-cookie字段不是utf-8字符串: {body}"))?
            .to_string();

        Ok(cookie)
    }

    pub async fn get_user_profile(&self) -> eyre::Result<UserProfile> {
        let cookie = self.app.get_config().read().cookie.clone();
        // 发送获取用户信息请求
        let request = self
            .api_client
            .read()
            .get("https://www.manhuagui.com/user/center/index")
            .header("cookie", cookie);
        let http_resp = request.send_with_timeout_msg().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == StatusCode::FOUND {
            return Err(eyre!("未登录、cookie已过期或cookie无效"));
        } else if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }

        let user_profile = UserProfile::from_html(&body).wrap_err("将body转换为UserProfile失败")?;
        Ok(user_profile)
    }

    pub async fn search(&self, keyword: &str, page_num: i64) -> eyre::Result<SearchResult> {
        let url = format!("https://www.manhuagui.com/s/{keyword}_p{page_num}.html");
        let request = self.api_client.read().get(url);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let search_result =
            SearchResult::from_html(&self.app, &body).wrap_err("将body转换为SearchResult失败")?;
        Ok(search_result)
    }

    pub async fn get_comic(&self, id: i64) -> eyre::Result<Comic> {
        let request = self
            .api_client
            .read()
            .get(format!("https://www.manhuagui.com/comic/{id}/"));
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        let comic = Comic::from_html(&self.app, &body).wrap_err("将body转换为Comic失败")?;

        Ok(comic)
    }

    pub async fn get_img_urls(&self, chapter_info: &ChapterInfo) -> eyre::Result<Vec<String>> {
        let comic_id = chapter_info.comic_id;
        let chapter_id = chapter_info.chapter_id;

        let url = format!("https://www.manhuagui.com/comic/{comic_id}/{chapter_id}.html");
        let request = self.api_client.read().get(url);
        let http_resp = request.send_with_timeout_msg().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }

        let decrypt_result = decrypt(&body).wrap_err("解密失败")?;

        let urls = decrypt_result
            .files
            .iter()
            .map(|file| format!("https://i.hamreus.com{}{file}", decrypt_result.path))
            .map(|url| url.trim_end_matches(".webp").to_string())
            .collect();

        Ok(urls)
    }

    pub async fn get_img_bytes(&self, url: &str) -> eyre::Result<Bytes> {
        // 发送下载图片请求
        let request = self
            .img_client
            .read()
            .get(url)
            .header("referer", "https://www.manhuagui.com/");
        let http_resp = request.send_with_timeout_msg().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        // 读取图片数据
        let image_data = http_resp.bytes().await?;

        Ok(image_data)
    }

    pub async fn get_favorite(&self, page_num: i64) -> eyre::Result<GetFavoriteResult> {
        let cookie = self.app.get_config().read().cookie.clone();
        // 发送获取收藏夹请求
        let url = format!("https://www.manhuagui.com/user/book/shelf/{page_num}");
        let request = self.api_client.read().get(url).header("cookie", cookie);
        let http_resp = request.send_with_timeout_msg().await?;
        // 检查http响应状态码
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(eyre!("预料之外的状态码({status}): {body}"));
        }
        // 解析html
        let get_favorite_result = GetFavoriteResult::from_html(&self.app, &body)
            .wrap_err("将body转换为GetFavoriteResult失败")?;
        Ok(get_favorite_result)
    }
}

fn create_api_client(app: &AppHandle) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1) // 指数为1，保证重试间隔为1秒不变
        .jitter(Jitter::Bounded) // 重试间隔在1秒左右波动
        .build_with_total_retry_duration(Duration::from_secs(5)); // 重试总时长为5秒

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(3)) // 每个请求超过3秒就超时
        .redirect(reqwest::redirect::Policy::none())
        .set_proxy(app, "api_client")
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_img_client(app: &AppHandle) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client = reqwest::ClientBuilder::new()
        .set_proxy(app, "img_client")
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

trait ClientBuilderExt {
    fn set_proxy(self, app: &AppHandle, client_name: &str) -> Self;
}

impl ClientBuilderExt for reqwest::ClientBuilder {
    fn set_proxy(self, app: &AppHandle, client_name: &str) -> reqwest::ClientBuilder {
        let proxy_mode = app.get_config().read().proxy_mode;
        match proxy_mode {
            ProxyMode::System => self,
            ProxyMode::NoProxy => self.no_proxy(),
            ProxyMode::Custom => {
                let config = app.get_config().inner().read();
                let proxy_host = &config.proxy_host;
                let proxy_port = &config.proxy_port;
                let proxy_url = format!("http://{proxy_host}:{proxy_port}");

                match reqwest::Proxy::all(&proxy_url).map_err(eyre::Report::from) {
                    Ok(proxy) => self.proxy(proxy),
                    Err(err) => {
                        let err_title = format!("{client_name}将`{proxy_url}`设为代理失败，将直连");
                        let string_chain = err.to_string_chain();
                        tracing::error!(err_title, message = string_chain);
                        self.no_proxy()
                    }
                }
            }
        }
    }
}
