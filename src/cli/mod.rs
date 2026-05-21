use std::{fmt, path::PathBuf, time::Duration};

use crate::retry::RetryLimit;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(2);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub headers: Vec<HeaderConfig>,
    pub interval: Duration,
    pub timeout: Duration,
    pub retry_limit: RetryLimit,
    pub once: bool,
    pub json: bool,
    pub debug: bool,
    pub log_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub auth: Option<AuthConfig>,
    pub ca_cert_path: Option<PathBuf>,
    pub insecure: bool,
    pub exit_on_match: Option<String>,
    pub max_events: Option<u64>,
    pub max_duration: Option<Duration>,
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
pub struct HeaderConfig {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Run(Box<Config>),
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
    InvalidHeader(String),
    InvalidMethod(String),
    InvalidNumber(String),
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
            Self::InvalidHeader(value) => {
                write!(formatter, "invalid header '{value}', expected NAME: VALUE")
            }
            Self::InvalidMethod(value) => write!(formatter, "invalid HTTP method '{value}'"),
            Self::InvalidNumber(value) => write!(formatter, "invalid number '{value}'"),
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
    let mut method = "GET".to_owned();
    let mut method_was_set = false;
    let mut body = None;
    let mut headers = Vec::new();
    let mut interval = DEFAULT_INTERVAL;
    let mut timeout = DEFAULT_TIMEOUT;
    let mut retry_limit = RetryLimit::Unlimited;
    let mut once = false;
    let mut json = false;
    let mut debug = false;
    let mut log_path = None;
    let mut output_path = None;
    let mut auth = None;
    let mut ca_cert_path = None;
    let mut insecure = false;
    let mut exit_on_match = None;
    let mut max_events = None;
    let mut max_duration = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliCommand::Help),
            "-V" | "--version" => return Ok(CliCommand::Version),
            "--method" => {
                method = parse_method(&next_value(&mut args, &arg)?)?;
                method_was_set = true;
            }
            "--body" => body = Some(next_value(&mut args, &arg)?),
            "--header" => headers.push(parse_header(&next_value(&mut args, &arg)?)?),
            "-i" | "--interval" => interval = parse_flag_duration(&mut args, &arg)?,
            "--timeout" => timeout = parse_flag_duration(&mut args, &arg)?,
            "--max-retries" => retry_limit = parse_flag_retry_limit(&mut args, &arg)?,
            "--once" => once = true,
            "--json" => json = true,
            "--debug" => debug = true,
            "--log" => log_path = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            "--output" => output_path = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            "--ca-cert" => ca_cert_path = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            "--insecure" => insecure = true,
            "--exit-on-match" => exit_on_match = Some(next_value(&mut args, &arg)?),
            "--max-events" => max_events = Some(parse_positive_u64(&next_value(&mut args, &arg)?)?),
            "--max-duration" => max_duration = Some(parse_flag_duration(&mut args, &arg)?),
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
    if let Some(max_duration) = max_duration {
        validate_nonzero(max_duration, "--max-duration")?;
    }
    if debug && log_path.is_none() {
        return Err(CliError::MissingRequiredFlag {
            flag: "--debug".to_owned(),
            required: "--log <PATH>".to_owned(),
        });
    }
    if body.is_some() && !method_was_set {
        method = "POST".to_owned();
    }
    let url = url.ok_or(CliError::MissingUrl)?;
    validate_url(&url)?;

    Ok(CliCommand::Run(Box::new(Config {
        url,
        method,
        body,
        headers,
        interval,
        timeout,
        retry_limit,
        once,
        json,
        debug,
        log_path,
        output_path,
        auth,
        ca_cert_path,
        insecure,
        exit_on_match,
        max_events,
        max_duration,
    })))
}

pub fn help_text(binary: &str) -> String {
    format!(
        "\
{binary} {version}

Follow changing HTTP content, similar to tail -f for URLs.

USAGE:
    {binary} [OPTIONS] <URL>

OPTIONS:
        --method <METHOD>         HTTP method [default: GET, or POST with --body]
        --body <TEXT>             Request body to send
        --header <NAME: VALUE>    Add an HTTP header; repeatable
    -i, --interval <DURATION>     Poll interval, for example 500ms, 2s, 1m
        --timeout <DURATION>      HTTP timeout per request [default: 10s]
        --max-retries <N|unlimited>
                                  Retry limit for consecutive retryable failures
        --once                    Fetch once and exit
        --json                    Emit each update as a JSON line
        --output <PATH>           Write stream output to a file instead of stdout
        --exit-on-match <REGEX>   Exit after emitted output matches a regex
        --max-events <N>          Exit after emitting N updates
        --max-duration <DURATION> Exit after the total runtime duration
        --debug                   Write debug logging; requires --log
        --log <PATH>              Debug log output file
        --ca-cert <PATH>          Add PEM or DER CA certificate roots for TLS
        --insecure                Disable TLS certificate verification
        --basic-auth <USER:PASS>  Send HTTP Basic authentication
        --token <TOKEN>           Send bearer token authentication
    -h, --help                    Print help
    -V, --version                 Print version
",
        version = env!("CARGO_PKG_VERSION")
    )
}

fn parse_method(value: &str) -> Result<String, CliError> {
    let method = value.to_ascii_uppercase();

    match method.as_str() {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => Ok(method),
        _ => Err(CliError::InvalidMethod(value.to_owned())),
    }
}

fn parse_header(value: &str) -> Result<HeaderConfig, CliError> {
    let (name, header_value) = value
        .split_once(':')
        .ok_or_else(|| CliError::InvalidHeader(value.to_owned()))?;
    let name = name.trim();
    let header_value = header_value.trim();

    if name.is_empty() || header_value.is_empty() {
        Err(CliError::InvalidHeader(value.to_owned()))
    } else {
        Ok(HeaderConfig {
            name: name.to_owned(),
            value: header_value.to_owned(),
        })
    }
}

fn parse_positive_u64(value: &str) -> Result<u64, CliError> {
    match value.parse::<u64>() {
        Ok(number) if number > 0 => Ok(number),
        _ => Err(CliError::InvalidNumber(value.to_owned())),
    }
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
                assert_eq!(config.method, "GET");
                assert_eq!(config.body, None);
                assert_eq!(config.headers, Vec::new());
                assert_eq!(config.interval, DEFAULT_INTERVAL);
                assert_eq!(config.timeout, DEFAULT_TIMEOUT);
                assert_eq!(config.retry_limit, RetryLimit::Unlimited);
                assert!(!config.debug);
                assert_eq!(config.log_path, None);
                assert_eq!(config.output_path, None);
                assert_eq!(config.auth, None);
                assert_eq!(config.ca_cert_path, None);
                assert!(!config.insecure);
                assert_eq!(config.exit_on_match, None);
                assert_eq!(config.max_events, None);
                assert_eq!(config.max_duration, None);
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
            "--method",
            "post",
            "--body",
            "{\"ok\":true}",
            "--header",
            "Content-Type: application/json",
            "--header",
            "X-Env: test",
            "--once",
            "--json",
            "--output",
            "streamtail.out",
            "--exit-on-match",
            "ready",
            "--max-events",
            "2",
            "--max-duration",
            "10s",
            "--debug",
            "--log",
            "streamtail.log",
            "--ca-cert",
            "ca.pem",
            "--insecure",
            "http://localhost",
        ])
        .expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(config.method, "POST");
                assert_eq!(config.body, Some("{\"ok\":true}".to_owned()));
                assert_eq!(
                    config.headers,
                    vec![
                        HeaderConfig {
                            name: "Content-Type".to_owned(),
                            value: "application/json".to_owned(),
                        },
                        HeaderConfig {
                            name: "X-Env".to_owned(),
                            value: "test".to_owned(),
                        }
                    ]
                );
                assert_eq!(config.interval, Duration::from_secs(1));
                assert_eq!(config.timeout, Duration::from_secs(5));
                assert_eq!(config.retry_limit, RetryLimit::Limited(3));
                assert!(config.once);
                assert!(config.json);
                assert!(config.debug);
                assert_eq!(config.log_path, Some(PathBuf::from("streamtail.log")));
                assert_eq!(config.output_path, Some(PathBuf::from("streamtail.out")));
                assert_eq!(config.ca_cert_path, Some(PathBuf::from("ca.pem")));
                assert!(config.insecure);
                assert_eq!(config.exit_on_match, Some("ready".to_owned()));
                assert_eq!(config.max_events, Some(2));
                assert_eq!(config.max_duration, Some(Duration::from_secs(10)));
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn body_defaults_method_to_post() {
        let command = parse(&["--body", "hello", "http://localhost"]).expect("valid args");

        match command {
            CliCommand::Run(config) => assert_eq!(config.method, "POST"),
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn rejects_invalid_header() {
        assert_eq!(
            parse(&["--header", "X-Test", "http://localhost"]),
            Err(CliError::InvalidHeader("X-Test".to_owned()))
        );
    }

    #[test]
    fn rejects_invalid_method() {
        assert_eq!(
            parse(&["--method", "TRACE", "http://localhost"]),
            Err(CliError::InvalidMethod("TRACE".to_owned()))
        );
    }

    #[test]
    fn rejects_zero_max_events() {
        assert_eq!(
            parse(&["--max-events", "0", "http://localhost"]),
            Err(CliError::InvalidNumber("0".to_owned()))
        );
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
