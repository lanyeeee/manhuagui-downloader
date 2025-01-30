use std::{io::Write, sync::Mutex};

use anyhow::Context;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tracing::Level;
use tracing_appender::{
    non_blocking,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{
    filter::{filter_fn, FilterExt, Targets},
    fmt::{layer, time::LocalTime},
    layer::SubscriberExt,
    registry,
    util::SubscriberInitExt,
    Layer,
};

use crate::events::LogEvent;

struct LogEventWriter {
    app: AppHandle,
}

impl Write for LogEventWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let log_string = String::from_utf8_lossy(buf);
        match serde_json::from_str::<LogEvent>(&log_string) {
            Ok(log_event) => {
                let _ = log_event.emit(&self.app);
            }
            Err(err) => {
                let log_string = log_string.to_string();
                let err_msg = err.to_string();
                tracing::error!(log_string, err_msg, "将日志字符串解析为LogEvent失败");
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn init(app: &AppHandle) -> anyhow::Result<()> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .context("获取app_data_dir目录失败")?;

    std::fs::create_dir_all(&app_data_dir)
        .context(format!("创建app_data_dir目录`{app_data_dir:?}`失败"))?;

    let lib_module_path = module_path!();
    let lib_target = lib_module_path.split("::").next().context(format!(
        "解析lib_target失败: lib_module_path={lib_module_path}"
    ))?;
    // 过滤掉来自其他库的日志
    let target_filter = Targets::new().with_target(lib_target, Level::TRACE);
    let logs_dir = logs_dir(app).context("获取日志目录失败")?;
    // 输出到文件
    let file_appender = RollingFileAppender::builder()
        .filename_prefix("manhuagui-downloader")
        .filename_suffix("log")
        .rotation(Rotation::DAILY)
        .build(logs_dir)
        .expect("创建RollingFileAppender失败");
    let (non_blocking_appender, guard) = non_blocking(file_appender);
    std::mem::forget(guard);
    let file_layer = layer()
        .with_writer(non_blocking_appender)
        .with_timer(LocalTime::rfc_3339())
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .with_filter(target_filter.clone());
    // 输出到控制台
    let console_layer = layer()
        .with_writer(std::io::stdout)
        .with_timer(LocalTime::rfc_3339())
        .with_file(true)
        .with_line_number(true)
        .with_filter(target_filter.clone());
    // 发送到前端
    let log_event_writer = Mutex::new(LogEventWriter { app: app.clone() });
    let log_event_layer = layer()
        .with_writer(log_event_writer)
        .with_timer(LocalTime::rfc_3339())
        .with_file(true)
        .with_line_number(true)
        .json()
        // 过滤掉来自这个文件的日志(LogEvent解析失败的日志)，避免无限递归
        .with_filter(target_filter.and(filter_fn(|metadata| {
            metadata.module_path() != Some(lib_module_path)
        })));

    registry()
        .with(file_layer)
        .with(console_layer)
        .with(log_event_layer)
        .init();

    Ok(())
}

pub fn logs_dir(app: &AppHandle) -> anyhow::Result<std::path::PathBuf> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .context("获取app_data_dir目录失败")?;
    Ok(app_data_dir.join("日志"))
}
