use std::{
    fmt,
    fs::{File, OpenOptions},
    io,
    io::Write,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

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
    MissingLogPath,
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch(err) => write!(formatter, "{err}"),
            Self::OutputIo(err) => write!(formatter, "failed to write output: {err}"),
            Self::LogIo(err) => write!(formatter, "failed to write debug log: {err}"),
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
    let fetcher =
        HttpFetcher::new(config.url.clone(), config.timeout).with_auth(config.auth.clone());
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    run_with(fetcher, &mut handle, &config)
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
    let backoff = Backoff::default();
    let mut logger = DebugLogger::open(config)?;

    log_debug(&mut logger, "debug logging enabled")?;
    log_debug(
        &mut logger,
        format!(
            "streamtail {} started; url={}; interval={:?}; timeout={:?}; once={}; json={}; auth={}",
            env!("CARGO_PKG_VERSION"),
            config.url,
            config.interval,
            config.timeout,
            config.once,
            config.json,
            config
                .auth
                .as_ref()
                .map(|auth| auth.scheme_name())
                .unwrap_or("none")
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
                let change = emit_change(&mut diff_engine, &mut renderer, &body)?;
                log_debug(
                    &mut logger,
                    format!(
                        "fetch attempt {fetch_attempts} succeeded; bytes={}; change={change}",
                        body.len()
                    ),
                )?;

                if config.once {
                    log_debug(&mut logger, "exiting after --once fetch")?;
                    return Ok(());
                }

                log_debug(
                    &mut logger,
                    format!("sleeping for {:?} before next poll", config.interval),
                )?;
                thread::sleep(config.interval);
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
) -> Result<&'static str, AppError>
where
    W: Write,
{
    match diff_engine.update(body) {
        Change::Unchanged => Ok("unchanged"),
        Change::Appended(text) => {
            renderer.emit(&text).map_err(AppError::OutputIo)?;
            Ok("appended")
        }
        Change::Reset(text) => {
            renderer.emit(&text).map_err(AppError::OutputIo)?;
            Ok("reset")
        }
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
            once: true,
            json: false,
            debug: false,
            log_path: None,
            auth: None,
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
