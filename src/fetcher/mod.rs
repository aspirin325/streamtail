use std::{fmt, time::Duration};

pub trait Fetcher {
    fn fetch(&self) -> Result<String, FetchError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    Retryable(String),
    Fatal(String),
}

impl FetchError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable(message.into())
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retryable(message) | Self::Fatal(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for FetchError {}

pub struct HttpFetcher {
    agent: ureq::Agent,
    url: String,
}

impl HttpFetcher {
    pub fn new(url: impl Into<String>, timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .timeout_write(timeout)
            .build();

        Self {
            agent,
            url: url.into(),
        }
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self) -> Result<String, FetchError> {
        match self.agent.get(&self.url).call() {
            Ok(response) => read_body(response),
            Err(ureq::Error::Status(status, _response)) => status_error(status),
            Err(ureq::Error::Transport(err)) => {
                Err(FetchError::retryable(format!("transport error: {err}")))
            }
        }
    }
}

fn read_body(response: ureq::Response) -> Result<String, FetchError> {
    response
        .into_string()
        .map_err(|err| FetchError::fatal(format!("failed to read response body: {err}")))
}

fn status_error(status: u16) -> Result<String, FetchError> {
    let message = format!("HTTP status {status}");

    if should_retry_status(status) {
        Err(FetchError::retryable(message))
    } else {
        Err(FetchError::fatal(message))
    }
}

fn should_retry_status(status: u16) -> bool {
    (500..=599).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_server_errors() {
        assert!(should_retry_status(500));
        assert!(should_retry_status(503));
    }

    #[test]
    fn does_not_retry_auth_errors() {
        assert!(!should_retry_status(401));
        assert!(!should_retry_status(403));
    }
}
