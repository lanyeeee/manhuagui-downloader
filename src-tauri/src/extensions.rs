use anyhow::anyhow;
use reqwest::Response;
use reqwest_middleware::RequestBuilder;
use scraper::error::SelectorErrorKind;

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

pub trait ToAnyhow<T> {
    fn to_anyhow(self) -> anyhow::Result<T>;
}

impl<T> ToAnyhow<T> for Result<T, SelectorErrorKind<'_>> {
    fn to_anyhow(self) -> anyhow::Result<T> {
        self.map_err(|e| anyhow!(e.to_string()))
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
                anyhow::Error::from(e).context(
                    "网络连接超时，可能是未使用代理或IP被封，请使用代理或切换代理线路后重试",
                )
            } else {
                anyhow::Error::from(e)
            }
        })
    }
}
