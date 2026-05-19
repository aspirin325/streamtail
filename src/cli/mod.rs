use std::{fmt, time::Duration};

use crate::retry::RetryLimit;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(2);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub url: String,
    pub interval: Duration,
    pub timeout: Duration,
    pub retry_limit: RetryLimit,
    pub once: bool,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Run(Config),
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    MissingUrl,
    MissingValue(String),
    UnexpectedArgument(String),
    UnknownFlag(String),
    InvalidDuration(String),
    InvalidRetryLimit(String),
    InvalidUrl(String),
    ZeroDuration(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUrl => write!(formatter, "missing URL"),
            Self::MissingValue(flag) => write!(formatter, "{flag} requires a value"),
            Self::UnexpectedArgument(arg) => write!(formatter, "unexpected argument '{arg}'"),
            Self::UnknownFlag(flag) => write!(formatter, "unknown flag '{flag}'"),
            Self::InvalidDuration(value) => write!(formatter, "invalid duration '{value}'"),
            Self::InvalidRetryLimit(value) => write!(formatter, "invalid retry limit '{value}'"),
            Self::InvalidUrl(url) => write!(formatter, "unsupported URL '{url}'"),
            Self::ZeroDuration(flag) => write!(formatter, "{flag} must be greater than zero"),
        }
    }
}

impl std::error::Error for CliError {}

pub fn parse_args<I>(args: I) -> Result<CliCommand, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let mut url = None;
    let mut interval = DEFAULT_INTERVAL;
    let mut timeout = DEFAULT_TIMEOUT;
    let mut retry_limit = RetryLimit::Unlimited;
    let mut once = false;
    let mut json = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliCommand::Help),
            "-V" | "--version" => return Ok(CliCommand::Version),
            "-i" | "--interval" => interval = parse_flag_duration(&mut args, &arg)?,
            "--timeout" => timeout = parse_flag_duration(&mut args, &arg)?,
            "--max-retries" => retry_limit = parse_flag_retry_limit(&mut args, &arg)?,
            "--once" => once = true,
            "--json" => json = true,
            _ if arg.starts_with('-') => return Err(CliError::UnknownFlag(arg)),
            _ if url.is_none() => url = Some(arg),
            _ => return Err(CliError::UnexpectedArgument(arg)),
        }
    }

    validate_nonzero(interval, "--interval")?;
    validate_nonzero(timeout, "--timeout")?;
    let url = url.ok_or(CliError::MissingUrl)?;
    validate_url(&url)?;

    Ok(CliCommand::Run(Config {
        url,
        interval,
        timeout,
        retry_limit,
        once,
        json,
    }))
}

pub fn help_text(binary: &str) -> String {
    format!(
        "\
{binary} {version}

Follow changing HTTP content, similar to tail -f for URLs.

USAGE:
    {binary} [OPTIONS] <URL>

OPTIONS:
    -i, --interval <DURATION>     Poll interval, for example 500ms, 2s, 1m
        --timeout <DURATION>      HTTP timeout per request [default: 10s]
        --max-retries <N|unlimited>
                                  Retry limit for consecutive retryable failures
        --once                    Fetch once and exit
        --json                    Emit each update as a JSON line
    -h, --help                    Print help
    -V, --version                 Print version
",
        version = env!("CARGO_PKG_VERSION")
    )
}

fn parse_flag_duration<I>(args: &mut I, flag: &str) -> Result<Duration, CliError>
where
    I: Iterator<Item = String>,
{
    let value = next_value(args, flag)?;
    parse_duration(&value)
}

fn parse_flag_retry_limit<I>(args: &mut I, flag: &str) -> Result<RetryLimit, CliError>
where
    I: Iterator<Item = String>,
{
    let value = next_value(args, flag)?;
    parse_retry_limit(&value)
}

fn next_value<I>(args: &mut I, flag: &str) -> Result<String, CliError>
where
    I: Iterator<Item = String>,
{
    let value = args
        .next()
        .ok_or_else(|| CliError::MissingValue(flag.to_owned()))?;

    if value.starts_with('-') {
        Err(CliError::MissingValue(flag.to_owned()))
    } else {
        Ok(value)
    }
}

fn validate_nonzero(duration: Duration, flag: &str) -> Result<(), CliError> {
    if duration.is_zero() {
        Err(CliError::ZeroDuration(flag.to_owned()))
    } else {
        Ok(())
    }
}

fn validate_url(url: &str) -> Result<(), CliError> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err(CliError::InvalidUrl(url.to_owned()))
    }
}

pub fn parse_duration(value: &str) -> Result<Duration, CliError> {
    let digits = value.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return Err(CliError::InvalidDuration(value.to_owned()));
    }

    let amount = value[..digits]
        .parse::<u64>()
        .map_err(|_| CliError::InvalidDuration(value.to_owned()))?;
    let unit = &value[digits..];

    match unit {
        "" | "s" => Ok(Duration::from_secs(amount)),
        "ms" => Ok(Duration::from_millis(amount)),
        "m" => checked_seconds(amount, 60, value),
        "h" => checked_seconds(amount, 3_600, value),
        _ => Err(CliError::InvalidDuration(value.to_owned())),
    }
}

pub fn parse_retry_limit(value: &str) -> Result<RetryLimit, CliError> {
    if value.eq_ignore_ascii_case("unlimited") {
        return Ok(RetryLimit::Unlimited);
    }

    value
        .parse::<u32>()
        .map(RetryLimit::Limited)
        .map_err(|_| CliError::InvalidRetryLimit(value.to_owned()))
}

fn checked_seconds(amount: u64, multiplier: u64, original: &str) -> Result<Duration, CliError> {
    amount
        .checked_mul(multiplier)
        .map(Duration::from_secs)
        .ok_or_else(|| CliError::InvalidDuration(original.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<CliCommand, CliError> {
        parse_args(values.iter().map(|value| value.to_string()))
    }

    #[test]
    fn parses_url_with_defaults() {
        let command = parse(&["https://example.com/logs"]).expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(config.url, "https://example.com/logs");
                assert_eq!(config.interval, DEFAULT_INTERVAL);
                assert_eq!(config.timeout, DEFAULT_TIMEOUT);
                assert_eq!(config.retry_limit, RetryLimit::Unlimited);
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_duration_units() {
        assert_eq!(parse_duration("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse_duration("2s"), Ok(Duration::from_secs(2)));
        assert_eq!(parse_duration("3m"), Ok(Duration::from_secs(180)));
        assert_eq!(parse_duration("1h"), Ok(Duration::from_secs(3_600)));
    }

    #[test]
    fn parses_flags() {
        let command = parse(&[
            "--interval",
            "1s",
            "--timeout",
            "5s",
            "--max-retries",
            "3",
            "--once",
            "--json",
            "http://localhost",
        ])
        .expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(config.interval, Duration::from_secs(1));
                assert_eq!(config.timeout, Duration::from_secs(5));
                assert_eq!(config.retry_limit, RetryLimit::Limited(3));
                assert!(config.once);
                assert!(config.json);
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn rejects_missing_url() {
        assert_eq!(parse(&[]), Err(CliError::MissingUrl));
    }

    #[test]
    fn rejects_non_http_url() {
        assert_eq!(
            parse(&["file:///tmp/logs"]),
            Err(CliError::InvalidUrl("file:///tmp/logs".to_owned()))
        );
    }
}
