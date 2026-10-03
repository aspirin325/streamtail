use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use streamtail::{
    cli::{self, AuthConfig, CliCommand, HeaderConfig},
    fetcher::{Fetcher, HttpFetcher},
};

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn serve_once(status: u16, body: &str) -> (String, JoinHandle<()>) {
    let (url, _requests, handle) = serve_once_with_request(status, body);

    (url, handle)
}

fn serve_once_with_request(status: u16, body: &str) -> (String, Receiver<String>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("server should bind");
    let address = listener.local_addr().expect("server address should exist");
    let body = body.to_owned();
    let (request_sender, requests) = mpsc::channel();

    let handle = thread::spawn(move || {
        if let Ok((mut stream, _peer)) = listener.accept() {
            let _timeout = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut buffer = [0_u8; 1024];
            let mut request_bytes = Vec::new();

            while let Ok(read) = stream.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..read]);

                let Some(headers_end) = request_bytes
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let body_start = headers_end + 4;
                let headers = String::from_utf8_lossy(&request_bytes[..headers_end]);
                let content_length = headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if request_bytes.len() >= body_start + content_length {
                    break;
                }
            }

            let request = String::from_utf8_lossy(&request_bytes).into_owned();
            let _send = request_sender.send(request);
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}",
                reason = reason_phrase(status),
                length = body.len()
            );
            let _write = stream.write_all(response.as_bytes());
        }
    });

    (format!("http://{address}/logs"), requests, handle)
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        401 => "Unauthorized",
        403 => "Forbidden",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

fn request_has_header(request: &str, expected_name: &str, expected_value: &str) -> bool {
    request
        .split("\r\n")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_once(':'))
        .any(|(name, value)| {
            name.eq_ignore_ascii_case(expected_name) && value.trim() == expected_value
        })
}

#[test]
fn fetcher_reads_successful_http_body() {
    let (url, handle) = serve_once(200, "hello");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2));

    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");

    assert_eq!(body, "hello");
}

#[test]
fn fetcher_marks_server_errors_retryable() {
    let (url, handle) = serve_once(500, "try later");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2));

    let error = fetcher.fetch().expect_err("500 should fail");
    handle.join().expect("server thread should finish");

    assert!(error.is_retryable());
}

#[test]
fn fetcher_marks_auth_errors_fatal() {
    let (url, handle) = serve_once(403, "forbidden");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2));

    let error = fetcher.fetch().expect_err("403 should fail");
    handle.join().expect("server thread should finish");

    assert!(!error.is_retryable());
}

#[test]
fn fetcher_retries_configured_statuses() {
    let (url, handle) = serve_once(404, "not found");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2)).with_retry_statuses(vec![404]);

    let error = fetcher.fetch().expect_err("404 should fail");
    handle.join().expect("server thread should finish");

    assert!(error.is_retryable());
}

#[test]
fn fetcher_sends_basic_auth_header() {
    let (url, requests, handle) = serve_once_with_request(200, "hello");
    let fetcher =
        HttpFetcher::new(url, Duration::from_secs(2)).with_auth(Some(AuthConfig::Basic {
            username: "alice".to_owned(),
            password: "secret".to_owned(),
        }));

    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");
    let request = requests.recv().expect("request should be captured");

    assert_eq!(body, "hello");
    assert!(request_has_header(
        &request,
        "Authorization",
        "Basic YWxpY2U6c2VjcmV0"
    ));
}

#[test]
fn fetcher_sends_bearer_token_header() {
    let (url, requests, handle) = serve_once_with_request(200, "hello");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2))
        .with_auth(Some(AuthConfig::BearerToken("abc123".to_owned())));

    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");
    let request = requests.recv().expect("request should be captured");

    assert_eq!(body, "hello");
    assert!(request_has_header(
        &request,
        "Authorization",
        "Bearer abc123"
    ));
}

#[test]
fn fetcher_sends_bearer_token_from_file() {
    let (url, requests, handle) = serve_once_with_request(200, "hello");
    let token_path = unique_output_path();
    std::fs::write(&token_path, "file-token\n").expect("token file should be writable");
    let command = cli::parse_args([
        "--once".to_owned(),
        "--token-file".to_owned(),
        token_path.display().to_string(),
        url,
    ])
    .expect("valid args");

    let CliCommand::Run(config) = command else {
        panic!("expected run command");
    };

    let fetcher = HttpFetcher::from_config(&config).expect("fetcher should build");
    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");
    let request = requests.recv().expect("request should be captured");
    let _ = std::fs::remove_file(&token_path);

    assert_eq!(body, "hello");
    assert!(request_has_header(
        &request,
        "Authorization",
        "Bearer file-token"
    ));
}

#[test]
fn fetcher_sends_bearer_token_from_env() {
    let (url, requests, handle) = serve_once_with_request(200, "hello");
    let env_name = format!("STREAMTAIL_TEST_TOKEN_{}", unique_id());
    std::env::set_var(&env_name, "env-token");
    let command = cli::parse_args([
        "--once".to_owned(),
        "--token-env".to_owned(),
        env_name.clone(),
        url,
    ])
    .expect("valid args");

    let CliCommand::Run(config) = command else {
        panic!("expected run command");
    };

    let fetcher = HttpFetcher::from_config(&config).expect("fetcher should build");
    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");
    let request = requests.recv().expect("request should be captured");
    std::env::remove_var(env_name);

    assert_eq!(body, "hello");
    assert!(request_has_header(
        &request,
        "Authorization",
        "Bearer env-token"
    ));
}

#[test]
fn fetcher_sends_method_headers_and_body() {
    let (url, requests, handle) = serve_once_with_request(200, "hello");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2))
        .with_method_body("POST", Some("{\"ok\":true}".to_owned()))
        .with_headers(vec![
            HeaderConfig {
                name: "Content-Type".to_owned(),
                value: "application/json".to_owned(),
            },
            HeaderConfig {
                name: "X-Test".to_owned(),
                value: "yes".to_owned(),
            },
        ]);

    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");
    let request = requests.recv().expect("request should be captured");

    assert_eq!(body, "hello");
    assert!(request.starts_with("POST /logs HTTP/1.1"));
    assert!(request_has_header(
        &request,
        "Content-Type",
        "application/json"
    ));
    assert!(request_has_header(&request, "X-Test", "yes"));
    assert!(request.contains("{\"ok\":true}"));
}

#[test]
fn fetcher_sends_user_agent_header() {
    let (url, requests, handle) = serve_once_with_request(200, "hello");
    let fetcher = HttpFetcher::new(url, Duration::from_secs(2)).with_user_agent("streamtail-test");

    let body = fetcher.fetch().expect("fetch should succeed");
    handle.join().expect("server thread should finish");
    let request = requests.recv().expect("request should be captured");

    assert_eq!(body, "hello");
    assert!(request_has_header(
        &request,
        "User-Agent",
        "streamtail-test"
    ));
}

#[test]
fn run_writes_stream_output_to_configured_file() {
    let (url, handle) = serve_once(200, "hello");
    let output_path = unique_output_path();
    let command = cli::parse_args([
        "--once".to_owned(),
        "--output".to_owned(),
        output_path.display().to_string(),
        url,
    ])
    .expect("valid args");

    let CliCommand::Run(config) = command else {
        panic!("expected run command");
    };

    streamtail::run(*config).expect("run should succeed");
    handle.join().expect("server thread should finish");

    let output = std::fs::read_to_string(&output_path).expect("output should be readable");
    let _ = std::fs::remove_file(&output_path);

    assert_eq!(output, "hello");
}

#[test]
fn run_does_not_write_color_codes_to_configured_file() {
    let (url, handle) = serve_once(200, "hello");
    let output_path = unique_output_path();
    let command = cli::parse_args([
        "--once".to_owned(),
        "--color".to_owned(),
        "red".to_owned(),
        "--output".to_owned(),
        output_path.display().to_string(),
        url,
    ])
    .expect("valid args");

    let CliCommand::Run(config) = command else {
        panic!("expected run command");
    };

    streamtail::run(*config).expect("run should succeed");
    handle.join().expect("server thread should finish");

    let output = std::fs::read_to_string(&output_path).expect("output should be readable");
    let _ = std::fs::remove_file(&output_path);

    assert_eq!(output, "hello");
}

#[test]
fn run_appends_stream_output_to_configured_file() {
    let (url, handle) = serve_once(200, "hello");
    let output_path = unique_output_path();
    std::fs::write(&output_path, "existing\n").expect("output should be writable");
    let command = cli::parse_args([
        "--once".to_owned(),
        "--output".to_owned(),
        output_path.display().to_string(),
        "--append-output".to_owned(),
        url,
    ])
    .expect("valid args");

    let CliCommand::Run(config) = command else {
        panic!("expected run command");
    };

    streamtail::run(*config).expect("run should succeed");
    handle.join().expect("server thread should finish");

    let output = std::fs::read_to_string(&output_path).expect("output should be readable");
    let _ = std::fs::remove_file(&output_path);

    assert_eq!(output, "existing\nhello");
}

fn unique_output_path() -> PathBuf {
    std::env::temp_dir().join(format!("streamtail-output-test-{}.txt", unique_id()))
}

fn unique_id() -> String {
    let sequence = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be valid")
        .as_nanos();

    format!("{}-{sequence}-{nanos}", std::process::id())
}
