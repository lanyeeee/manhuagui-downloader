use std::time::Duration;

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::{
    config::Config,
    types::{SearchResult, UserProfile},
};

#[derive(Clone)]
pub struct ManhuaguiClient {
    app: AppHandle,
    api_client: ClientWithMiddleware,
}

impl ManhuaguiClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client();
        Self { app, api_client }
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
            .send()
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
            .send()
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
        let http_resp = self.api_client.get(url).send().await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("预料之外的状态码({status}): {body}"));
        }
        let search_result =
            SearchResult::from_html(&body).context("将body转换为SearchResult失败")?;
        Ok(search_result)
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
