use std::panic::Location;

use eyre::{eyre, EyreHandler};
use serde::Serialize;
use specta::Type;
use tracing_error::SpanTrace;

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Type, Serialize)]
pub struct CommandError {
    pub err_title: String,
    pub message: String,
}

impl CommandError {
    pub fn from<E>(err_title: &str, err: E) -> Self
    where
        E: Into<eyre::Report>,
    {
        let message = format!("{:?}", err.into());
        tracing::error!(err_title, message);
        Self {
            err_title: err_title.to_string(),
            message,
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

struct CustomEyreHandler {
    span_trace: SpanTrace,
    location: Option<&'static Location<'static>>,
}

impl EyreHandler for CustomEyreHandler {
    fn debug(
        &self,
        error: &(dyn std::error::Error + 'static),
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        use std::fmt::Write;

        let mut buf = String::new();

        writeln!(&mut buf, "Error:")?;
        writeln!(&mut buf, "   0: {error}")?;

        let mut current = error.source();
        let mut i = 1;
        while let Some(cause) = current {
            writeln!(&mut buf, "   {i}: {cause}")?;
            current = cause.source();
            i += 1;
        }

        if let Some(loc) = self.location {
            writeln!(&mut buf, "Location:")?;
            writeln!(&mut buf, "   at {}:{}", loc.file(), loc.line())?;
        }

        let span_trace = format!("{}", self.span_trace);
        if !span_trace.is_empty() {
            writeln!(&mut buf, "SpanTrace:")?;
            writeln!(&mut buf, "{span_trace}")?;
        }

        write!(f, "{}", buf.trim_end())?;

        Ok(())
    }
    fn track_caller(&mut self, location: &'static Location<'static>) {
        self.location = Some(location);
    }
}

pub fn install_custom_eyre_handler() -> eyre::Result<()> {
    eyre::set_hook(Box::new(|_error| {
        Box::new(CustomEyreHandler {
            span_trace: SpanTrace::capture(),
            location: None,
        })
    }))?;
    Ok(())
}
