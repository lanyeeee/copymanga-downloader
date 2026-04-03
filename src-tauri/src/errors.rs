use eyre::eyre;
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

pub type RiskControlResult<T> = Result<T, RiskControlError>;

#[derive(Debug)]
pub enum RiskControlError {
    Report(eyre::Report),
    RiskControl(String),
}

impl<E> From<E> for RiskControlError
where
    E: Into<eyre::Report>,
    Result<(), E>: eyre::WrapErr<(), E>,
{
    fn from(err: E) -> Self {
        RiskControlError::Report(err.into())
    }
}

impl From<RiskControlError> for eyre::Report {
    fn from(err: RiskControlError) -> Self {
        match err {
            RiskControlError::Report(err) => err,
            RiskControlError::RiskControl(body) => eyre!(body),
        }
    }
}

pub type GetUserProfileResult<T> = Result<T, GetUserProfileError>;

#[derive(Debug)]
pub enum GetUserProfileError {
    Report(eyre::Report),
    TokenErrorOrExpired,
}

impl<E> From<E> for GetUserProfileError
where
    E: Into<eyre::Report>,
    Result<(), E>: eyre::WrapErr<(), E>,
{
    fn from(err: E) -> Self {
        GetUserProfileError::Report(err.into())
    }
}

impl From<GetUserProfileError> for eyre::Report {
    fn from(err: GetUserProfileError) -> Self {
        match err {
            GetUserProfileError::Report(err) => err,
            GetUserProfileError::TokenErrorOrExpired => eyre!("token错误或已过期"),
        }
    }
}
