use std::{fmt, path::PathBuf, time::Duration};

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
    pub debug: bool,
    pub log_path: Option<PathBuf>,
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfig {
    Basic { username: String, password: String },
    BearerToken(String),
}

impl AuthConfig {
    pub fn scheme_name(&self) -> &'static str {
        match self {
            Self::Basic { .. } => "basic",
            Self::BearerToken(_) => "bearer",
        }
    }
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
    InvalidBasicAuth(String),
    ZeroDuration(String),
    MissingRequiredFlag { flag: String, required: String },
    ConflictingAuthFlags,
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
            Self::InvalidBasicAuth(value) => write!(
                formatter,
                "invalid basic auth value '{value}', expected USER:PASSWORD"
            ),
            Self::ZeroDuration(flag) => write!(formatter, "{flag} must be greater than zero"),
            Self::MissingRequiredFlag { flag, required } => {
                write!(formatter, "{flag} requires {required}")
            }
            Self::ConflictingAuthFlags => {
                write!(
                    formatter,
                    "--basic-auth and --token cannot be used together"
                )
            }
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
    let mut debug = false;
    let mut log_path = None;
    let mut auth = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliCommand::Help),
            "-V" | "--version" => return Ok(CliCommand::Version),
            "-i" | "--interval" => interval = parse_flag_duration(&mut args, &arg)?,
            "--timeout" => timeout = parse_flag_duration(&mut args, &arg)?,
            "--max-retries" => retry_limit = parse_flag_retry_limit(&mut args, &arg)?,
            "--once" => once = true,
            "--json" => json = true,
            "--debug" => debug = true,
            "--log" => log_path = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            "--basic-auth" => {
                set_auth(&mut auth, parse_basic_auth(&next_value(&mut args, &arg)?)?)?;
            }
            "--token" => {
                set_auth(
                    &mut auth,
                    AuthConfig::BearerToken(next_value(&mut args, &arg)?),
                )?;
            }
            _ if arg.starts_with('-') => return Err(CliError::UnknownFlag(arg)),
            _ if url.is_none() => url = Some(arg),
            _ => return Err(CliError::UnexpectedArgument(arg)),
        }
    }

    validate_nonzero(interval, "--interval")?;
    validate_nonzero(timeout, "--timeout")?;
    if debug && log_path.is_none() {
        return Err(CliError::MissingRequiredFlag {
            flag: "--debug".to_owned(),
            required: "--log <PATH>".to_owned(),
        });
    }
    let url = url.ok_or(CliError::MissingUrl)?;
    validate_url(&url)?;

    Ok(CliCommand::Run(Config {
        url,
        interval,
        timeout,
        retry_limit,
        once,
        json,
        debug,
        log_path,
        auth,
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
        --debug                   Write debug logging; requires --log
        --log <PATH>              Debug log output file
        --basic-auth <USER:PASS>  Send HTTP Basic authentication
        --token <TOKEN>           Send bearer token authentication
    -h, --help                    Print help
    -V, --version                 Print version
",
        version = env!("CARGO_PKG_VERSION")
    )
}

fn set_auth(auth: &mut Option<AuthConfig>, value: AuthConfig) -> Result<(), CliError> {
    if auth.is_some() {
        Err(CliError::ConflictingAuthFlags)
    } else {
        *auth = Some(value);
        Ok(())
    }
}

fn parse_basic_auth(value: &str) -> Result<AuthConfig, CliError> {
    let (username, password) = value
        .split_once(':')
        .ok_or_else(|| CliError::InvalidBasicAuth(value.to_owned()))?;

    if username.is_empty() {
        Err(CliError::InvalidBasicAuth(value.to_owned()))
    } else {
        Ok(AuthConfig::Basic {
            username: username.to_owned(),
            password: password.to_owned(),
        })
    }
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
                assert!(!config.debug);
                assert_eq!(config.log_path, None);
                assert_eq!(config.auth, None);
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
            "--debug",
            "--log",
            "streamtail.log",
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
                assert!(config.debug);
                assert_eq!(config.log_path, Some(PathBuf::from("streamtail.log")));
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_version_flag() {
        assert_eq!(parse(&["--version"]), Ok(CliCommand::Version));
    }

    #[test]
    fn parses_basic_auth() {
        let command =
            parse(&["--basic-auth", "alice:secret", "http://localhost"]).expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.auth,
                    Some(AuthConfig::Basic {
                        username: "alice".to_owned(),
                        password: "secret".to_owned(),
                    })
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_token_auth() {
        let command = parse(&["--token", "abc123", "http://localhost"]).expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.auth,
                    Some(AuthConfig::BearerToken("abc123".to_owned()))
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn rejects_debug_without_log() {
        assert_eq!(
            parse(&["--debug", "http://localhost"]),
            Err(CliError::MissingRequiredFlag {
                flag: "--debug".to_owned(),
                required: "--log <PATH>".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_invalid_basic_auth() {
        assert_eq!(
            parse(&["--basic-auth", "alice", "http://localhost"]),
            Err(CliError::InvalidBasicAuth("alice".to_owned()))
        );
    }

    #[test]
    fn rejects_conflicting_auth_flags() {
        assert_eq!(
            parse(&[
                "--basic-auth",
                "alice:secret",
                "--token",
                "abc123",
                "http://localhost"
            ]),
            Err(CliError::ConflictingAuthFlags)
        );
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
