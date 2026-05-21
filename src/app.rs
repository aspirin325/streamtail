use std::{
    fmt,
    fs::{File, OpenOptions},
    io,
    io::Write,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use regex::Regex;

use crate::{
    cli::Config,
    diff::{Change, DiffEngine},
    fetcher::{FetchError, Fetcher, HttpFetcher},
    renderer::{OutputMode, Renderer},
    retry::Backoff,
};

#[derive(Debug)]
pub enum AppError {
    Fetch(FetchError),
    OutputIo(io::Error),
    LogIo(io::Error),
    InvalidExitPattern(regex::Error),
    MissingLogPath,
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch(err) => write!(formatter, "{err}"),
            Self::OutputIo(err) => write!(formatter, "failed to write output: {err}"),
            Self::LogIo(err) => write!(formatter, "failed to write debug log: {err}"),
            Self::InvalidExitPattern(err) => {
                write!(formatter, "invalid --exit-on-match regex: {err}")
            }
            Self::MissingLogPath => write!(formatter, "--debug requires --log <PATH>"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<FetchError> for AppError {
    fn from(err: FetchError) -> Self {
        Self::Fetch(err)
    }
}

pub fn run(config: Config) -> Result<(), AppError> {
    let fetcher = HttpFetcher::from_config(&config)?;

    if let Some(path) = &config.output_path {
        let mut options = OpenOptions::new();
        options.create(true).write(true);

        if config.append_output {
            options.append(true);
        } else {
            options.truncate(true);
        }

        let mut file = options.open(path).map_err(AppError::OutputIo)?;
        run_with(fetcher, &mut file, &config)
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        run_with(fetcher, &mut handle, &config)
    }
}

pub fn run_with<F, W>(fetcher: F, writer: &mut W, config: &Config) -> Result<(), AppError>
where
    F: Fetcher,
    W: Write,
{
    let mut diff_engine = DiffEngine::default();
    let mut renderer = Renderer::new(writer, output_mode(config));
    let mut consecutive_failures = 0_u32;
    let mut fetch_attempts = 0_u64;
    let mut emitted_events = 0_u64;
    let mut baseline_pending = config.follow_from_end;
    let backoff = Backoff::default();
    let mut logger = DebugLogger::open(config)?;
    let exit_on_match = match &config.exit_on_match {
        Some(pattern) => Some(Regex::new(pattern).map_err(AppError::InvalidExitPattern)?),
        None => None,
    };
    let started_at = Instant::now();

    log_debug(&mut logger, "debug logging enabled")?;
    log_debug(
        &mut logger,
        format!(
            "streamtail {} started; url={}; method={}; interval={:?}; timeout={:?}; once={}; follow_from_end={}; json={}; auth={}; netrc={}",
            env!("CARGO_PKG_VERSION"),
            config.url,
            config.method,
            config.interval,
            config.timeout,
            config.once,
            config.follow_from_end,
            config.json,
            config
                .auth
                .as_ref()
                .map(|auth| auth.scheme_name())
                .unwrap_or("none"),
            config.netrc
        ),
    )?;

    loop {
        fetch_attempts = fetch_attempts.saturating_add(1);
        log_debug(
            &mut logger,
            format!("fetch attempt {fetch_attempts} started"),
        )?;

        match fetcher.fetch() {
            Ok(body) => {
                consecutive_failures = 0;
                let change = if baseline_pending {
                    baseline_pending = false;
                    diff_engine.update(&body);
                    EmittedChange {
                        kind: "baseline",
                        text: None,
                    }
                } else {
                    emit_change(&mut diff_engine, &mut renderer, &body)?
                };
                if change.text.is_some() {
                    emitted_events = emitted_events.saturating_add(1);
                }
                log_debug(
                    &mut logger,
                    format!(
                        "fetch attempt {fetch_attempts} succeeded; bytes={}; change={}; emitted_events={emitted_events}",
                        body.len(),
                        change.kind
                    ),
                )?;

                if config.once {
                    log_debug(&mut logger, "exiting after --once fetch")?;
                    return Ok(());
                }
                if let Some(text) = &change.text {
                    if let Some(pattern) = &exit_on_match {
                        if pattern.is_match(text) {
                            log_debug(
                                &mut logger,
                                "exiting because --exit-on-match matched emitted output",
                            )?;
                            return Ok(());
                        }
                    }
                }
                if config
                    .max_events
                    .is_some_and(|limit| emitted_events >= limit)
                {
                    log_debug(&mut logger, "exiting after --max-events limit")?;
                    return Ok(());
                }
                if max_duration_reached(config.max_duration, started_at) {
                    log_debug(&mut logger, "exiting after --max-duration limit")?;
                    return Ok(());
                }

                let sleep_for = sleep_duration(config.interval, config.max_duration, started_at);
                if sleep_for.is_zero() {
                    log_debug(&mut logger, "exiting after --max-duration limit")?;
                    return Ok(());
                }

                log_debug(
                    &mut logger,
                    format!("sleeping for {sleep_for:?} before next poll"),
                )?;
                thread::sleep(sleep_for);
            }
            Err(err)
                if err.is_retryable() && config.retry_limit.allows_retry(consecutive_failures) =>
            {
                consecutive_failures = consecutive_failures.saturating_add(1);
                let delay = backoff.delay(consecutive_failures);
                log_debug(
                    &mut logger,
                    format!(
                        "fetch attempt {fetch_attempts} failed with retryable error: {err}; consecutive_failures={consecutive_failures}; sleeping for {delay:?}"
                    ),
                )?;
                thread::sleep(delay);
            }
            Err(err) => {
                log_debug(
                    &mut logger,
                    format!("fetch attempt {fetch_attempts} failed with fatal error: {err}"),
                )?;
                return Err(AppError::Fetch(err));
            }
        }
    }
}

fn max_duration_reached(max_duration: Option<Duration>, started_at: Instant) -> bool {
    max_duration.is_some_and(|limit| started_at.elapsed() >= limit)
}

fn sleep_duration(
    interval: Duration,
    max_duration: Option<Duration>,
    started_at: Instant,
) -> Duration {
    match max_duration.and_then(|limit| limit.checked_sub(started_at.elapsed())) {
        Some(remaining) => interval.min(remaining),
        None if max_duration.is_some() => Duration::ZERO,
        None => interval,
    }
}

fn output_mode(config: &Config) -> OutputMode {
    if config.json {
        OutputMode::Json
    } else {
        OutputMode::Text
    }
}

fn emit_change<W>(
    diff_engine: &mut DiffEngine,
    renderer: &mut Renderer<W>,
    body: &str,
) -> Result<EmittedChange, AppError>
where
    W: Write,
{
    match diff_engine.update(body) {
        Change::Unchanged => Ok(EmittedChange {
            kind: "unchanged",
            text: None,
        }),
        Change::Appended(text) => {
            renderer.emit(&text).map_err(AppError::OutputIo)?;
            Ok(EmittedChange::new("appended", text))
        }
        Change::Reset(text) => {
            renderer.emit(&text).map_err(AppError::OutputIo)?;
            Ok(EmittedChange::new("reset", text))
        }
    }
}

struct EmittedChange {
    kind: &'static str,
    text: Option<String>,
}

impl EmittedChange {
    fn new(kind: &'static str, text: String) -> Self {
        let text = if text.is_empty() { None } else { Some(text) };

        Self { kind, text }
    }
}

struct DebugLogger {
    file: File,
}

impl DebugLogger {
    fn open(config: &Config) -> Result<Option<Self>, AppError> {
        if !config.debug {
            return Ok(None);
        }

        let path = config.log_path.as_ref().ok_or(AppError::MissingLogPath)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(AppError::LogIo)?;

        Ok(Some(Self { file }))
    }

    fn log(&mut self, message: &str) -> Result<(), AppError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        writeln!(
            self.file,
            "[{}.{:03}] {message}",
            timestamp.as_secs(),
            timestamp.subsec_millis()
        )
        .map_err(AppError::LogIo)
    }
}

fn log_debug(logger: &mut Option<DebugLogger>, message: impl AsRef<str>) -> Result<(), AppError> {
    if let Some(logger) = logger {
        logger.log(message.as_ref())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::VecDeque, time::Duration};

    use super::*;
    use crate::{cli::Config, retry::RetryLimit};

    struct SequenceFetcher {
        responses: RefCell<VecDeque<Result<String, FetchError>>>,
    }

    impl SequenceFetcher {
        fn new(responses: Vec<Result<String, FetchError>>) -> Self {
            Self {
                responses: RefCell::new(VecDeque::from(responses)),
            }
        }
    }

    impl Fetcher for SequenceFetcher {
        fn fetch(&self) -> Result<String, FetchError> {
            self.responses
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Ok(String::new()))
        }
    }

    fn test_config() -> Config {
        Config {
            url: "http://localhost/logs".to_owned(),
            interval: Duration::from_millis(1),
            timeout: Duration::from_secs(1),
            retry_limit: RetryLimit::Limited(0),
            retry_statuses: Vec::new(),
            once: true,
            follow_from_end: false,
            json: false,
            debug: false,
            log_path: None,
            output_path: None,
            append_output: false,
            auth: None,
            netrc: false,
            method: "GET".to_owned(),
            body: None,
            headers: Vec::new(),
            user_agent: None,
            proxy: None,
            ca_cert_path: None,
            insecure: false,
            exit_on_match: None,
            max_events: None,
            max_duration: None,
        }
    }

    #[test]
    fn run_once_writes_initial_body() {
        let fetcher = SequenceFetcher::new(vec![Ok("hello".to_owned())]);
        let mut output = Vec::new();

        run_with(fetcher, &mut output, &test_config()).expect("run should succeed");

        assert_eq!(output, b"hello");
    }

    #[test]
    fn run_once_writes_json_when_enabled() {
        let fetcher = SequenceFetcher::new(vec![Ok("hello\n".to_owned())]);
        let mut config = test_config();
        config.json = true;
        let mut output = Vec::new();

        run_with(fetcher, &mut output, &config).expect("run should succeed");

        assert_eq!(
            output,
            br#"{"data":"hello\n"}
"#
        );
        assert_eq!(output.last(), Some(&b'\n'));
    }

    #[test]
    fn debug_logging_writes_to_configured_log_file() {
        let fetcher = SequenceFetcher::new(vec![Ok("hello".to_owned())]);
        let mut config = test_config();
        config.debug = true;
        config.log_path = Some(unique_log_path());
        let mut output = Vec::new();

        run_with(fetcher, &mut output, &config).expect("run should succeed");

        let log_path = config.log_path.as_ref().expect("log path should exist");
        let log = std::fs::read_to_string(log_path).expect("log should be readable");
        let _ = std::fs::remove_file(log_path);

        assert!(log.contains("debug logging enabled"));
        assert!(log.contains("fetch attempt 1 succeeded"));
        assert!(log.contains("change=appended"));
    }

    #[test]
    fn exits_when_emitted_output_matches_pattern() {
        let fetcher = SequenceFetcher::new(vec![Ok("ready".to_owned()), Ok("later".to_owned())]);
        let mut config = test_config();
        config.once = false;
        config.exit_on_match = Some("ready".to_owned());
        let mut output = Vec::new();

        run_with(fetcher, &mut output, &config).expect("run should succeed");

        assert_eq!(output, b"ready");
    }

    #[test]
    fn follow_from_end_skips_initial_body() {
        let fetcher =
            SequenceFetcher::new(vec![Ok("hello".to_owned()), Ok("hello world".to_owned())]);
        let mut config = test_config();
        config.once = false;
        config.follow_from_end = true;
        config.max_events = Some(1);
        let mut output = Vec::new();

        run_with(fetcher, &mut output, &config).expect("run should succeed");

        assert_eq!(output, b" world");
    }

    #[test]
    fn exits_after_max_events() {
        let fetcher = SequenceFetcher::new(vec![Ok("one".to_owned()), Ok("onetwo".to_owned())]);
        let mut config = test_config();
        config.once = false;
        config.max_events = Some(2);
        let mut output = Vec::new();

        run_with(fetcher, &mut output, &config).expect("run should succeed");

        assert_eq!(output, b"onetwo");
    }

    #[test]
    fn rejects_invalid_exit_pattern() {
        let fetcher = SequenceFetcher::new(vec![Ok("hello".to_owned())]);
        let mut config = test_config();
        config.exit_on_match = Some("[".to_owned());
        let mut output = Vec::new();

        let error = run_with(fetcher, &mut output, &config).expect_err("regex should fail");

        assert!(matches!(error, AppError::InvalidExitPattern(_)));
    }

    fn unique_log_path() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be valid")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "streamtail-test-{}-{nanos}.log",
            std::process::id()
        ))
    }
}
