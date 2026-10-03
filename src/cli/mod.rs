use std::{fmt, path::PathBuf, time::Duration};

use crate::{color::Color, retry::RetryLimit};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(2);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_URLS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub sources: Vec<SourceConfig>,
    pub method: String,
    pub body: Option<String>,
    pub headers: Vec<HeaderConfig>,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub interval: Duration,
    pub timeout: Duration,
    pub retry_limit: RetryLimit,
    pub retry_statuses: Vec<u16>,
    pub once: bool,
    pub follow_from_end: bool,
    pub json: bool,
    pub debug: bool,
    pub log_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub append_output: bool,
    pub auth: Option<AuthConfig>,
    pub netrc: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub insecure: bool,
    pub exit_on_match: Option<String>,
    pub max_events: Option<u64>,
    pub max_duration: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfig {
    pub url: String,
    pub color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfig {
    Basic { username: String, password: String },
    BearerToken(String),
    BearerTokenFile(PathBuf),
    BearerTokenEnv(String),
}

impl AuthConfig {
    pub fn scheme_name(&self) -> &'static str {
        match self {
            Self::Basic { .. } => "basic",
            Self::BearerToken(_) => "bearer",
            Self::BearerTokenFile(_) => "bearer-file",
            Self::BearerTokenEnv(_) => "bearer-env",
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
    InvalidStatusCode(String),
    InvalidColor(String),
    ZeroDuration(String),
    MissingRequiredFlag { flag: String, required: String },
    ConflictingAuthFlags,
    ConflictingColorFlags,
    TooManyUrls { limit: usize },
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
            Self::InvalidStatusCode(value) => {
                write!(formatter, "invalid HTTP status code '{value}'")
            }
            Self::InvalidColor(value) => write!(
                formatter,
                "invalid color '{value}', expected black, red, green, yellow, blue, magenta, cyan, white, or bright-*"
            ),
            Self::ZeroDuration(flag) => write!(formatter, "{flag} must be greater than zero"),
            Self::MissingRequiredFlag { flag, required } => {
                write!(formatter, "{flag} requires {required}")
            }
            Self::ConflictingAuthFlags => {
                write!(
                    formatter,
                    "only one authentication source can be used at a time"
                )
            }
            Self::ConflictingColorFlags => {
                write!(formatter, "--color and --no-color cannot be used together")
            }
            Self::TooManyUrls { limit } => write!(formatter, "at most {limit} URLs are supported"),
        }
    }
}

impl std::error::Error for CliError {}

pub fn parse_args<I>(args: I) -> Result<CliCommand, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let mut sources = Vec::new();
    let mut method = "GET".to_owned();
    let mut method_was_set = false;
    let mut body = None;
    let mut headers = Vec::new();
    let mut user_agent = None;
    let mut proxy = None;
    let mut interval = DEFAULT_INTERVAL;
    let mut timeout = DEFAULT_TIMEOUT;
    let mut retry_limit = RetryLimit::Unlimited;
    let mut retry_statuses = Vec::new();
    let mut once = false;
    let mut follow_from_end = false;
    let mut json = false;
    let mut pending_color = None;
    let mut color_was_set = false;
    let mut no_color = false;
    let mut debug = false;
    let mut log_path = None;
    let mut output_path = None;
    let mut append_output = false;
    let mut auth = None;
    let mut netrc = false;
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
            "--user-agent" => user_agent = Some(next_value(&mut args, &arg)?),
            "--proxy" => proxy = Some(next_value(&mut args, &arg)?),
            "-i" | "--interval" => interval = parse_flag_duration(&mut args, &arg)?,
            "--timeout" => timeout = parse_flag_duration(&mut args, &arg)?,
            "--max-retries" => retry_limit = parse_flag_retry_limit(&mut args, &arg)?,
            "--retry-status" => {
                retry_statuses.push(parse_status_code(&next_value(&mut args, &arg)?)?);
            }
            "--once" => once = true,
            "-f" | "--follow-from-end" => follow_from_end = true,
            "--json" => json = true,
            "--color" => {
                if no_color {
                    return Err(CliError::ConflictingColorFlags);
                }
                if pending_color.is_some() {
                    return Err(CliError::MissingRequiredFlag {
                        flag: "--color".to_owned(),
                        required: "<URL>".to_owned(),
                    });
                }
                pending_color = Some(parse_color(&next_value(&mut args, &arg)?)?);
                color_was_set = true;
            }
            "--no-color" => no_color = true,
            "--debug" => debug = true,
            "--log" => log_path = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            "--output" => output_path = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            "--append-output" => append_output = true,
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
            "--token-file" => {
                set_auth(
                    &mut auth,
                    AuthConfig::BearerTokenFile(PathBuf::from(next_value(&mut args, &arg)?)),
                )?;
            }
            "--token-env" => {
                set_auth(
                    &mut auth,
                    AuthConfig::BearerTokenEnv(next_value(&mut args, &arg)?),
                )?;
            }
            "--netrc" => netrc = true,
            _ if arg.starts_with('-') => return Err(CliError::UnknownFlag(arg)),
            _ => {
                validate_url(&arg)?;
                sources.push(SourceConfig {
                    url: arg,
                    color: pending_color.take(),
                });
            }
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
    if append_output && output_path.is_none() {
        return Err(CliError::MissingRequiredFlag {
            flag: "--append-output".to_owned(),
            required: "--output <PATH>".to_owned(),
        });
    }
    if netrc && auth.is_some() {
        return Err(CliError::ConflictingAuthFlags);
    }
    if no_color && color_was_set {
        return Err(CliError::ConflictingColorFlags);
    }
    if pending_color.is_some() {
        return Err(CliError::MissingRequiredFlag {
            flag: "--color".to_owned(),
            required: "<URL>".to_owned(),
        });
    }
    if body.is_some() && !method_was_set {
        method = "POST".to_owned();
    }
    if sources.is_empty() {
        return Err(CliError::MissingUrl);
    }
    if sources.len() > MAX_URLS {
        return Err(CliError::TooManyUrls { limit: MAX_URLS });
    }
    if !no_color {
        assign_source_colors(&mut sources);
    }

    Ok(CliCommand::Run(Box::new(Config {
        sources,
        method,
        body,
        headers,
        user_agent,
        proxy,
        interval,
        timeout,
        retry_limit,
        retry_statuses,
        once,
        follow_from_end,
        json,
        debug,
        log_path,
        output_path,
        append_output,
        auth,
        netrc,
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
    {binary} [OPTIONS] <URL> [URL ...]

OPTIONS:
        --method <METHOD>         HTTP method [default: GET, or POST with --body]
        --body <TEXT>             Request body to send
        --header <NAME: VALUE>    Add an HTTP header; repeatable
        --user-agent <VALUE>      Set the HTTP User-Agent header
        --proxy <URL>             Send requests through an HTTP or HTTPS proxy
    -i, --interval <DURATION>     Poll interval, for example 500ms, 2s, 1m
        --timeout <DURATION>      HTTP timeout per request [default: 10s]
        --max-retries <N|unlimited>
                                  Retry limit for consecutive retryable failures
        --retry-status <CODE>     Retry an additional HTTP status code; repeatable
        --once                    Fetch once and exit
    -f, --follow-from-end         Start after the first fetched snapshot
        --json                    Emit each update as a JSON line
        --color <COLOR>           Color the next URL's text output on stdout
        --no-color                Disable terminal color formatting for all URLs
        --output <PATH>           Write stream output to a file instead of stdout
        --append-output           Append to --output instead of replacing it
        --exit-on-match <REGEX>   Exit after emitted output matches a regex
        --max-events <N>          Exit after emitting N updates
        --max-duration <DURATION> Exit after the total runtime duration
        --debug                   Write debug logging; requires --log
        --log <PATH>              Debug log output file
        --ca-cert <PATH>          Use PEM or DER CA certificate roots for TLS
        --insecure                Disable TLS certificate verification
        --basic-auth <USER:PASS>  Send HTTP Basic authentication
        --token <TOKEN>           Send bearer token authentication
        --token-file <PATH>       Read bearer token authentication from a file
        --token-env <NAME>        Read bearer token authentication from an env var
        --netrc                   Use matching credentials from ~/.netrc
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

fn parse_status_code(value: &str) -> Result<u16, CliError> {
    match value.parse::<u16>() {
        Ok(status) if (100..=599).contains(&status) => Ok(status),
        _ => Err(CliError::InvalidStatusCode(value.to_owned())),
    }
}

fn parse_color(value: &str) -> Result<Color, CliError> {
    Color::parse(value).ok_or_else(|| CliError::InvalidColor(value.to_owned()))
}

fn assign_source_colors(sources: &mut [SourceConfig]) {
    if sources.len() < 2 {
        return;
    }

    let mut used = sources
        .iter()
        .filter_map(|source| source.color)
        .collect::<Vec<_>>();

    for source in sources.iter_mut().filter(|source| source.color.is_none()) {
        let color = auto_colors()
            .iter()
            .copied()
            .find(|color| !used.contains(color))
            .unwrap_or(Color::Cyan);
        source.color = Some(color);
        used.push(color);
    }
}

fn auto_colors() -> &'static [Color; MAX_URLS] {
    &[
        Color::Cyan,
        Color::Magenta,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::BrightRed,
    ]
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
                assert_eq!(
                    config.sources,
                    vec![SourceConfig {
                        url: "https://example.com/logs".to_owned(),
                        color: None,
                    }]
                );
                assert_eq!(config.method, "GET");
                assert_eq!(config.body, None);
                assert_eq!(config.headers, Vec::new());
                assert_eq!(config.user_agent, None);
                assert_eq!(config.proxy, None);
                assert_eq!(config.interval, DEFAULT_INTERVAL);
                assert_eq!(config.timeout, DEFAULT_TIMEOUT);
                assert_eq!(config.retry_limit, RetryLimit::Unlimited);
                assert!(config.retry_statuses.is_empty());
                assert!(!config.follow_from_end);
                assert!(!config.debug);
                assert_eq!(config.log_path, None);
                assert_eq!(config.output_path, None);
                assert!(!config.append_output);
                assert_eq!(config.auth, None);
                assert!(!config.netrc);
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
            "--user-agent",
            "streamtail-test/1.0",
            "--proxy",
            "http://proxy.local:8080",
            "--retry-status",
            "429",
            "--retry-status",
            "404",
            "--once",
            "--follow-from-end",
            "--json",
            "--color",
            "bright-cyan",
            "--output",
            "streamtail.out",
            "--append-output",
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
                assert_eq!(config.user_agent, Some("streamtail-test/1.0".to_owned()));
                assert_eq!(config.proxy, Some("http://proxy.local:8080".to_owned()));
                assert_eq!(config.interval, Duration::from_secs(1));
                assert_eq!(config.timeout, Duration::from_secs(5));
                assert_eq!(config.retry_limit, RetryLimit::Limited(3));
                assert_eq!(config.retry_statuses, vec![429, 404]);
                assert!(config.once);
                assert!(config.follow_from_end);
                assert!(config.json);
                assert_eq!(
                    config.sources,
                    vec![SourceConfig {
                        url: "http://localhost".to_owned(),
                        color: Some(Color::BrightCyan),
                    }]
                );
                assert!(config.debug);
                assert_eq!(config.log_path, Some(PathBuf::from("streamtail.log")));
                assert_eq!(config.output_path, Some(PathBuf::from("streamtail.out")));
                assert!(config.append_output);
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
    fn parses_multiple_urls_with_auto_colors() {
        let command = parse(&[
            "http://localhost/one",
            "http://localhost/two",
            "http://localhost/three",
        ])
        .expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.sources,
                    vec![
                        SourceConfig {
                            url: "http://localhost/one".to_owned(),
                            color: Some(Color::Cyan),
                        },
                        SourceConfig {
                            url: "http://localhost/two".to_owned(),
                            color: Some(Color::Magenta),
                        },
                        SourceConfig {
                            url: "http://localhost/three".to_owned(),
                            color: Some(Color::Green),
                        },
                    ]
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_per_url_color_overrides() {
        let command = parse(&[
            "--color",
            "red",
            "http://localhost/one",
            "http://localhost/two",
        ])
        .expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.sources,
                    vec![
                        SourceConfig {
                            url: "http://localhost/one".to_owned(),
                            color: Some(Color::Red),
                        },
                        SourceConfig {
                            url: "http://localhost/two".to_owned(),
                            color: Some(Color::Cyan),
                        },
                    ]
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_no_color_for_all_urls() {
        let command = parse(&["--no-color", "http://localhost/one", "http://localhost/two"])
            .expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.sources,
                    vec![
                        SourceConfig {
                            url: "http://localhost/one".to_owned(),
                            color: None,
                        },
                        SourceConfig {
                            url: "http://localhost/two".to_owned(),
                            color: None,
                        },
                    ]
                );
            }
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
    fn rejects_invalid_retry_status() {
        assert_eq!(
            parse(&["--retry-status", "99", "http://localhost"]),
            Err(CliError::InvalidStatusCode("99".to_owned()))
        );
    }

    #[test]
    fn rejects_invalid_color() {
        assert_eq!(
            parse(&["--color", "orange", "http://localhost"]),
            Err(CliError::InvalidColor("orange".to_owned()))
        );
    }

    #[test]
    fn rejects_conflicting_color_flags() {
        assert_eq!(
            parse(&["--color", "red", "--no-color", "http://localhost"]),
            Err(CliError::ConflictingColorFlags)
        );
    }

    #[test]
    fn rejects_color_without_url() {
        assert_eq!(
            parse(&["--color", "red"]),
            Err(CliError::MissingRequiredFlag {
                flag: "--color".to_owned(),
                required: "<URL>".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_more_than_six_urls() {
        assert_eq!(
            parse(&[
                "http://localhost/1",
                "http://localhost/2",
                "http://localhost/3",
                "http://localhost/4",
                "http://localhost/5",
                "http://localhost/6",
                "http://localhost/7",
            ]),
            Err(CliError::TooManyUrls { limit: MAX_URLS })
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
    fn parses_token_file_auth() {
        let command =
            parse(&["--token-file", "token.txt", "http://localhost"]).expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.auth,
                    Some(AuthConfig::BearerTokenFile(PathBuf::from("token.txt")))
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_token_env_auth() {
        let command =
            parse(&["--token-env", "STREAMTAIL_TOKEN", "http://localhost"]).expect("valid args");

        match command {
            CliCommand::Run(config) => {
                assert_eq!(
                    config.auth,
                    Some(AuthConfig::BearerTokenEnv("STREAMTAIL_TOKEN".to_owned()))
                );
            }
            _ => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_netrc_auth() {
        let command = parse(&["--netrc", "http://localhost"]).expect("valid args");

        match command {
            CliCommand::Run(config) => assert!(config.netrc),
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
    fn rejects_append_output_without_output() {
        assert_eq!(
            parse(&["--append-output", "http://localhost"]),
            Err(CliError::MissingRequiredFlag {
                flag: "--append-output".to_owned(),
                required: "--output <PATH>".to_owned(),
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
    fn rejects_netrc_with_other_auth() {
        assert_eq!(
            parse(&["--netrc", "--token", "abc123", "http://localhost"]),
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
