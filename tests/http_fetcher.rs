use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use streamtail::{
    cli::{self, AuthConfig, CliCommand, HeaderConfig},
    fetcher::{Fetcher, HttpFetcher},
};

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
            let _timeout = stream.set_read_timeout(Some(Duration::from_millis(100)));
            let mut buffer = [0_u8; 1024];
            let mut request_bytes = Vec::new();

            while let Ok(read) = stream.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..read]);
                if read < buffer.len() {
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
    assert!(request.contains("Authorization: Basic YWxpY2U6c2VjcmV0"));
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
    assert!(request.contains("Authorization: Bearer abc123"));
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
    assert!(request.contains("Content-Type: application/json"));
    assert!(request.contains("X-Test: yes"));
    assert!(request.contains("{\"ok\":true}"));
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

fn unique_output_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be valid")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "streamtail-output-test-{}-{nanos}.txt",
        std::process::id()
    ))
}
