use anyhow::anyhow;
use serde::Serialize;
use specta::Type;

use crate::extensions::AnyhowErrorToStringChain;

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Type)]
pub struct CommandError(String);
impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:#}", self.0))
    }
}
impl<E> From<E> for CommandError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into().to_string_chain())
    }
}

pub type CopyMangaResult<T> = Result<T, CopyMangaError>;

#[derive(Debug)]
pub enum CopyMangaError {
    Anyhow(anyhow::Error),
    RiskControl(RiskControlError), // 风控
}

impl<E> From<E> for CopyMangaError
where
    E: Into<anyhow::Error>,
    Result<(), E>: anyhow::Context<(), E>,
{
    fn from(err: E) -> Self {
        CopyMangaError::Anyhow(err.into())
    }
}

impl From<CopyMangaError> for anyhow::Error {
    fn from(err: CopyMangaError) -> Self {
        match err {
            CopyMangaError::Anyhow(err) => err,
            CopyMangaError::RiskControl(err) => match err {
                RiskControlError::Register(err) => anyhow!(err),
                RiskControlError::Login(err) => anyhow!(err),
                RiskControlError::GetUserProfile(err) => anyhow!(err),
                RiskControlError::Search(err) => anyhow!(err),
                RiskControlError::GetComic(err) => anyhow!(err),
                RiskControlError::GetChapter(err) => anyhow!(err),
                RiskControlError::GetChapters(err) => anyhow!(err),
                RiskControlError::GetFavorite(err) => anyhow!(err),
            },
        }
    }
}

impl From<RiskControlError> for CopyMangaError {
    fn from(err: RiskControlError) -> Self {
        CopyMangaError::RiskControl(err)
    }
}

#[derive(Debug)]
pub enum RiskControlError {
    Register(String),
    Login(String),
    GetUserProfile(String),
    Search(String),
    GetComic(String),
    GetChapter(String),
    GetChapters(String),
    GetFavorite(String),
}
