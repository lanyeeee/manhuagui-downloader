use std::collections::HashMap;

use anyhow::{anyhow, Context};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct DecryptResult {
    /// 漫画id
    pub bid: i64,
    /// 漫画名
    pub bname: String,
    /// 封面图片名称 (漫画id.jpg)
    pub bpic: String,
    /// 章节id
    pub cid: i64,
    /// 章节名
    pub cname: String,
    /// 章节图片名 (xxx.jpg.webp)
    pub files: Vec<String>,
    /// 是否已完结
    pub finished: bool,
    /// 章节图片数量
    pub len: i64,
    /// `https://i.hamreus.com{path}{file}` 为图片url
    pub path: String,
    /// 不知道有啥用，都是1
    pub status: i64,
    /// 不知道有啥用，都为""
    #[serde(rename = "block_cc")]
    pub block_cc: String,
    /// 下一个章节id，如果没有则为0
    pub next_id: i64,
    /// 上一个章节id，如果没有则为0
    pub prev_id: i64,
    /// 应该是凭证之类的东西，用不到
    pub sl: Sl,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sl {
    /// 看起来是时间戳
    e: i64,
    /// 不知道有啥用 (`agr1qEo-yIKXp_vfN6HbYg`)
    m: String,
}

pub fn decrypt(html: &str) -> anyhow::Result<DecryptResult> {
    let (function, a, c, data) = extract_decryption_data(html)?;

    let dict = create_dict(a, c, &data);

    let js = create_js(&function, &dict).context("生成js失败")?;

    let decrypt_result = create_decrypt_result(&js).context("生成DecryptResult失败")?;

    Ok(decrypt_result)
}

fn extract_decryption_data(html: &str) -> anyhow::Result<(String, i32, i32, Vec<String>)> {
    let re =
        Regex::new(r"^.*}\('(.*)',(\d*),(\d*),'([\w|+/=]*)'.*$").context("正则表达式编译失败")?;

    let captures = re.captures(html).context("正则表达式没有匹配到内容")?;

    let function = captures
        .get(1)
        .context("匹配到的内容没有function部分")?
        .as_str()
        .to_string();

    let a = captures
        .get(2)
        .context("匹配到的内容没有a部分")?
        .as_str()
        .parse::<i32>()
        .context("将a部分转换为整数失败")?;

    let c = captures
        .get(3)
        .context("匹配到的内容没有c部分")?
        .as_str()
        .parse::<i32>()
        .context("将c部分转换为整数失败")?;

    let compressed_data = captures
        .get(4)
        .context("匹配到的内容没有compressed_data部分")?
        .as_str();

    let decompressed_data =
        lz_str::decompress_from_base64(compressed_data).ok_or(anyhow!("lzstring解压缩失败"))?;
    let decompressed =
        String::from_utf16(&decompressed_data).context("lzstring解压缩后的数据不是utf-16字符串")?;

    let data = decompressed
        .split('|')
        .map(str::to_string)
        .collect::<Vec<_>>();

    Ok((function, a, c, data))
}

#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
fn create_dict(a: i32, mut c: i32, data: &[String]) -> HashMap<String, String> {
    fn itr(value: i32, num: i32, a: i32) -> String {
        const D: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        if value <= 0 {
            return String::new();
        }
        let mut result = itr(value / num, num, a);
        result.push(D.chars().nth((value % a) as usize).unwrap());
        result
    }

    fn tr(value: i32, num: i32, a: i32) -> String {
        let tmp = itr(value, num, a);
        if tmp.is_empty() {
            "0".to_string()
        } else {
            tmp
        }
    }

    fn e(c: i32, a: i32) -> String {
        let prefix = if c < a { String::new() } else { e(c / a, a) };

        let suffix = if c % a > 35 {
            ((c % a + 29) as u8 as char).to_string()
        } else {
            tr(c % a, 36, a)
        };

        format!("{prefix}{suffix}")
    }

    let mut dict = HashMap::new();
    while c > 0 {
        c -= 1;
        let key = e(c, a);
        let value = if data[c as usize].is_empty() {
            key.clone()
        } else {
            data[c as usize].clone()
        };
        dict.insert(key, value);
    }

    dict
}

fn create_js(function: &str, dict: &HashMap<String, String>) -> anyhow::Result<String> {
    let re = Regex::new(r"(\b\w+\b)").context("正则表达式编译失败")?;

    let splits = re.split(function).collect::<Vec<_>>();

    let matches = re
        .find_iter(function)
        .map(|m| m.as_str())
        .collect::<Vec<_>>();

    let mut pieces = Vec::new();
    for i in 0..splits.len() {
        pieces.push(splits[i]);
        if i < matches.len() {
            pieces.push(matches[i]);
        }
    }

    let mut js = String::new();
    for x in pieces {
        if let Some(val) = dict.get(x) {
            js.push_str(val);
        } else {
            js.push_str(x);
        }
    }

    Ok(js)
}

fn create_decrypt_result(js: &str) -> anyhow::Result<DecryptResult> {
    let re = Regex::new(r"^.*\((\{.*})\).*$").context("正则表达式编译失败")?;

    let captures = re.captures(js).context("正则表达式没有匹配到内容")?;

    let json_str = captures
        .get(1)
        .context("匹配到的内容没有json部分")?
        .as_str();

    let decrypt_result = serde_json::from_str::<DecryptResult>(json_str)
        .context("将解密后的数据转换为DecryptResult失败")?;

    Ok(decrypt_result)
}
