use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};

use streamtail::{
    cli::AuthConfig,
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
            let mut buffer = [0_u8; 1024];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
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
