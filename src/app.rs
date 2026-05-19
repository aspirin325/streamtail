use std::{fmt, io, io::Write, thread};

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
    Io(io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch(err) => write!(formatter, "{err}"),
            Self::Io(err) => write!(formatter, "failed to write output: {err}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<FetchError> for AppError {
    fn from(err: FetchError) -> Self {
        Self::Fetch(err)
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

pub fn run(config: Config) -> Result<(), AppError> {
    let fetcher = HttpFetcher::new(config.url.clone(), config.timeout);
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
    let backoff = Backoff::default();

    loop {
        match fetcher.fetch() {
            Ok(body) => {
                consecutive_failures = 0;
                emit_change(&mut diff_engine, &mut renderer, &body)?;

                if config.once {
                    return Ok(());
                }

                thread::sleep(config.interval);
            }
            Err(err)
                if err.is_retryable() && config.retry_limit.allows_retry(consecutive_failures) =>
            {
                consecutive_failures = consecutive_failures.saturating_add(1);
                thread::sleep(backoff.delay(consecutive_failures));
            }
            Err(err) => return Err(AppError::Fetch(err)),
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
) -> Result<(), AppError>
where
    W: Write,
{
    match diff_engine.update(body) {
        Change::Unchanged => Ok(()),
        Change::Appended(text) | Change::Reset(text) => renderer.emit(&text).map_err(AppError::Io),
    }
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
}
