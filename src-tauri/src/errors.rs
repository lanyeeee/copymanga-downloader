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
