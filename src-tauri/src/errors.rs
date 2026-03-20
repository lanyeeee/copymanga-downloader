use anyhow::anyhow;
use serde::Serialize;
use specta::Type;

use crate::extensions::AnyhowErrorToStringChain;

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Type, Serialize)]
pub struct CommandError {
    pub err_title: String,
    pub err_message: String,
}

impl CommandError {
    pub fn from<E>(err_title: &str, err: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let string_chain = err.into().to_string_chain();
        tracing::error!(err_title, message = string_chain);
        Self {
            err_title: err_title.to_string(),
            err_message: string_chain,
        }
    }
}

pub type RiskControlResult<T> = Result<T, RiskControlError>;

#[derive(Debug)]
pub enum RiskControlError {
    Anyhow(anyhow::Error),
    RiskControl(String),
}

impl<E> From<E> for RiskControlError
where
    E: Into<anyhow::Error>,
    Result<(), E>: anyhow::Context<(), E>,
{
    fn from(err: E) -> Self {
        RiskControlError::Anyhow(err.into())
    }
}

impl From<RiskControlError> for anyhow::Error {
    fn from(err: RiskControlError) -> Self {
        match err {
            RiskControlError::Anyhow(err) => err,
            RiskControlError::RiskControl(body) => anyhow!(body),
        }
    }
}

pub type GetUserProfileResult<T> = Result<T, GetUserProfileError>;

#[derive(Debug)]
pub enum GetUserProfileError {
    Anyhow(anyhow::Error),
    TokenErrorOrExpired,
}

impl<E> From<E> for GetUserProfileError
where
    E: Into<anyhow::Error>,
    Result<(), E>: anyhow::Context<(), E>,
{
    fn from(err: E) -> Self {
        GetUserProfileError::Anyhow(err.into())
    }
}

impl From<GetUserProfileError> for anyhow::Error {
    fn from(err: GetUserProfileError) -> Self {
        match err {
            GetUserProfileError::Anyhow(err) => err,
            GetUserProfileError::TokenErrorOrExpired => anyhow!("token错误或已过期"),
        }
    }
}
