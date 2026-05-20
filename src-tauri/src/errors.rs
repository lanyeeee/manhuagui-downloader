use serde::Serialize;
use specta::Type;

use crate::extensions::ReportToStringChain;

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Type, Serialize)]
pub struct CommandError {
    pub err_title: String,
    pub err_message: String,
}

impl CommandError {
    pub fn from<E>(err_title: &str, err: E) -> Self
    where
        E: Into<eyre::Report>,
    {
        let string_chain = err.into().to_string_chain();
        tracing::error!(err_title, message = string_chain);
        Self {
            err_title: err_title.to_string(),
            err_message: string_chain,
        }
    }
}
