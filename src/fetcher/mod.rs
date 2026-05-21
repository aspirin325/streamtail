use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use base64::{engine::general_purpose, Engine as _};

use crate::cli::{AuthConfig, Config, HeaderConfig};

pub trait Fetcher {
    fn fetch(&self) -> Result<String, FetchError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    Retryable(String),
    Fatal(String),
}

impl FetchError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable(message.into())
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retryable(message) | Self::Fatal(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for FetchError {}

pub struct HttpFetcher {
    agent: ureq::Agent,
    url: String,
    method: String,
    body: Option<String>,
    headers: Vec<HeaderConfig>,
    user_agent: Option<String>,
    auth: Option<AuthConfig>,
    retry_statuses: Vec<u16>,
}

impl HttpFetcher {
    pub fn new(url: impl Into<String>, timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .timeout_write(timeout)
            .build();

        Self {
            agent,
            url: url.into(),
            method: "GET".to_owned(),
            body: None,
            headers: Vec::new(),
            user_agent: None,
            auth: None,
            retry_statuses: Vec::new(),
        }
    }

    pub fn from_config(config: &Config) -> Result<Self, FetchError> {
        let agent = build_agent(
            config.timeout,
            config.ca_cert_path.as_ref(),
            config.insecure,
            config.proxy.as_deref(),
        )?;
        let auth = resolve_auth(&config.url, config.auth.as_ref(), config.netrc)?;

        Ok(Self {
            agent,
            url: config.url.clone(),
            method: config.method.clone(),
            body: config.body.clone(),
            headers: config.headers.clone(),
            user_agent: config.user_agent.clone(),
            auth,
            retry_statuses: config.retry_statuses.clone(),
        })
    }

    pub fn with_auth(mut self, auth: Option<AuthConfig>) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_headers(mut self, headers: Vec<HeaderConfig>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    pub fn with_method_body(mut self, method: impl Into<String>, body: Option<String>) -> Self {
        self.method = method.into();
        self.body = body;
        self
    }

    pub fn with_retry_statuses(mut self, retry_statuses: Vec<u16>) -> Self {
        self.retry_statuses = retry_statuses;
        self
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self) -> Result<String, FetchError> {
        let request = self.request()?;
        let response = if let Some(body) = &self.body {
            request.send_string(body)
        } else {
            request.call()
        };

        match response {
            Ok(response) => read_body(response),
            Err(ureq::Error::Status(status, _response)) => {
                status_error(status, &self.retry_statuses)
            }
            Err(ureq::Error::Transport(err)) => {
                Err(FetchError::retryable(format!("transport error: {err}")))
            }
        }
    }
}

impl HttpFetcher {
    fn request(&self) -> Result<ureq::Request, FetchError> {
        let mut request = self.agent.request(&self.method, &self.url);

        for header in &self.headers {
            request = request.set(&header.name, &header.value);
        }

        if let Some(user_agent) = &self.user_agent {
            request = request.set("User-Agent", user_agent);
        }

        if let Some(auth) = &self.auth {
            let auth = resolve_explicit_auth(auth)?;
            request = apply_auth_header(request, &auth);
        }

        Ok(request)
    }
}

fn apply_auth_header(request: ureq::Request, auth: &AuthConfig) -> ureq::Request {
    match auth {
        AuthConfig::Basic { username, password } => {
            let credentials = format!("{username}:{password}");
            let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
            request.set("Authorization", &format!("Basic {encoded}"))
        }
        AuthConfig::BearerToken(token) => request.set("Authorization", &format!("Bearer {token}")),
        AuthConfig::BearerTokenFile(_) | AuthConfig::BearerTokenEnv(_) => request,
    }
}

fn build_agent(
    timeout: Duration,
    ca_cert_path: Option<&PathBuf>,
    insecure: bool,
    proxy: Option<&str>,
) -> Result<ureq::Agent, FetchError> {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout_write(timeout);

    if let Some(proxy_url) = proxy {
        let proxy = ureq::Proxy::new(proxy_url)
            .map_err(|err| FetchError::fatal(format!("invalid proxy URL '{proxy_url}': {err}")))?;
        builder = builder.proxy(proxy);
    }

    if insecure || ca_cert_path.is_some() {
        Ok(builder
            .tls_config(Arc::new(build_tls_config(ca_cert_path, insecure)?))
            .build())
    } else {
        Ok(builder.build())
    }
}

fn resolve_auth(
    url: &str,
    auth: Option<&AuthConfig>,
    netrc: bool,
) -> Result<Option<AuthConfig>, FetchError> {
    if let Some(auth) = auth {
        return resolve_explicit_auth(auth).map(Some);
    }

    if netrc {
        return load_netrc_auth(url).map(Some);
    }

    Ok(None)
}

fn resolve_explicit_auth(auth: &AuthConfig) -> Result<AuthConfig, FetchError> {
    match auth {
        AuthConfig::Basic { username, password } => Ok(AuthConfig::Basic {
            username: username.clone(),
            password: password.clone(),
        }),
        AuthConfig::BearerToken(token) => Ok(AuthConfig::BearerToken(token.clone())),
        AuthConfig::BearerTokenFile(path) => {
            let token = fs::read_to_string(path).map_err(|err| {
                FetchError::fatal(format!(
                    "failed to read bearer token from {}: {err}",
                    path.display()
                ))
            })?;

            Ok(AuthConfig::BearerToken(clean_token(
                &token,
                "--token-file",
            )?))
        }
        AuthConfig::BearerTokenEnv(name) => {
            let token = env::var(name).map_err(|err| {
                FetchError::fatal(format!(
                    "failed to read bearer token from environment variable {name}: {err}"
                ))
            })?;

            Ok(AuthConfig::BearerToken(clean_token(&token, "--token-env")?))
        }
    }
}

fn clean_token(token: &str, source: &str) -> Result<String, FetchError> {
    let token = token.trim();

    if token.is_empty() {
        Err(FetchError::fatal(format!(
            "{source} resolved to an empty token"
        )))
    } else {
        Ok(token.to_owned())
    }
}

fn build_tls_config(
    ca_cert_path: Option<&PathBuf>,
    insecure: bool,
) -> Result<ureq::rustls::ClientConfig, FetchError> {
    let provider = ureq::rustls::crypto::ring::default_provider();
    let builder = ureq::rustls::ClientConfig::builder_with_provider(provider.clone().into())
        .with_protocol_versions(&[&ureq::rustls::version::TLS12, &ureq::rustls::version::TLS13])
        .map_err(|err| FetchError::fatal(format!("failed to configure TLS: {err}")))?;

    if insecure {
        let verifier = InsecureVerifier {
            supported_schemes: provider
                .signature_verification_algorithms
                .supported_schemes(),
        };

        return Ok(builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth());
    }

    let mut root_store = ureq::rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    if let Some(path) = ca_cert_path {
        let certs = load_ca_certs(path)?;
        let (valid, _invalid) = root_store.add_parsable_certificates(certs);

        if valid == 0 {
            return Err(FetchError::fatal(format!(
                "no valid CA certificates found in {}",
                path.display()
            )));
        }
    }

    Ok(builder
        .with_root_certificates(root_store)
        .with_no_client_auth())
}

fn load_ca_certs(
    path: &Path,
) -> Result<Vec<ureq::rustls::pki_types::CertificateDer<'static>>, FetchError> {
    let bytes = fs::read(path).map_err(|err| {
        FetchError::fatal(format!(
            "failed to read CA certificate {}: {err}",
            path.display()
        ))
    })?;

    if bytes
        .windows(b"-----BEGIN CERTIFICATE-----".len())
        .any(|window| window == b"-----BEGIN CERTIFICATE-----")
    {
        parse_pem_certs(&bytes, path)
    } else {
        Ok(vec![ureq::rustls::pki_types::CertificateDer::from(bytes)])
    }
}

fn parse_pem_certs(
    bytes: &[u8],
    path: &Path,
) -> Result<Vec<ureq::rustls::pki_types::CertificateDer<'static>>, FetchError> {
    let text = std::str::from_utf8(bytes).map_err(|err| {
        FetchError::fatal(format!(
            "failed to parse CA certificate {} as PEM: {err}",
            path.display()
        ))
    })?;
    let mut certs = Vec::new();
    let mut rest = text;

    while let Some(begin) = rest.find("-----BEGIN CERTIFICATE-----") {
        let after_begin = &rest[begin + "-----BEGIN CERTIFICATE-----".len()..];
        let Some(end) = after_begin.find("-----END CERTIFICATE-----") else {
            return Err(FetchError::fatal(format!(
                "unterminated PEM certificate in {}",
                path.display()
            )));
        };
        let body = &after_begin[..end];
        let encoded: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        let der = general_purpose::STANDARD.decode(encoded).map_err(|err| {
            FetchError::fatal(format!(
                "failed to decode PEM certificate in {}: {err}",
                path.display()
            ))
        })?;
        certs.push(ureq::rustls::pki_types::CertificateDer::from(der));
        rest = &after_begin[end + "-----END CERTIFICATE-----".len()..];
    }

    if certs.is_empty() {
        Err(FetchError::fatal(format!(
            "no PEM certificates found in {}",
            path.display()
        )))
    } else {
        Ok(certs)
    }
}

#[derive(Debug)]
struct InsecureVerifier {
    supported_schemes: Vec<ureq::rustls::SignatureScheme>,
}

impl ureq::rustls::client::danger::ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &ureq::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[ureq::rustls::pki_types::CertificateDer<'_>],
        _server_name: &ureq::rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: ureq::rustls::pki_types::UnixTime,
    ) -> Result<ureq::rustls::client::danger::ServerCertVerified, ureq::rustls::Error> {
        Ok(ureq::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &ureq::rustls::pki_types::CertificateDer<'_>,
        _dss: &ureq::rustls::DigitallySignedStruct,
    ) -> Result<ureq::rustls::client::danger::HandshakeSignatureValid, ureq::rustls::Error> {
        Ok(ureq::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &ureq::rustls::pki_types::CertificateDer<'_>,
        _dss: &ureq::rustls::DigitallySignedStruct,
    ) -> Result<ureq::rustls::client::danger::HandshakeSignatureValid, ureq::rustls::Error> {
        Ok(ureq::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<ureq::rustls::SignatureScheme> {
        self.supported_schemes.clone()
    }
}

fn read_body(response: ureq::Response) -> Result<String, FetchError> {
    response
        .into_string()
        .map_err(|err| FetchError::fatal(format!("failed to read response body: {err}")))
}

fn load_netrc_auth(url: &str) -> Result<AuthConfig, FetchError> {
    let host = host_from_url(url)
        .ok_or_else(|| FetchError::fatal(format!("failed to read host from URL '{url}'")))?;
    let path = default_netrc_path()
        .ok_or_else(|| FetchError::fatal("failed to locate home directory for .netrc"))?;
    let content = fs::read_to_string(&path).map_err(|err| {
        FetchError::fatal(format!(
            "failed to read .netrc from {}: {err}",
            path.display()
        ))
    })?;
    let credentials = parse_netrc_credentials(&content, &host).ok_or_else(|| {
        FetchError::fatal(format!("no .netrc credentials found for host '{host}'"))
    })?;

    Ok(AuthConfig::Basic {
        username: credentials.login,
        password: credentials.password,
    })
}

fn default_netrc_path() -> Option<PathBuf> {
    if cfg!(windows) {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .map(|home| home.join("_netrc"))
            .or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join("_netrc"))
            })
    } else {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".netrc"))
    }
}

fn host_from_url(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://")?.1;
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);

    if let Some(rest) = host_port.strip_prefix('[') {
        return rest.find(']').map(|end| rest[..end].to_owned());
    }

    let host = host_port.split(':').next().unwrap_or(host_port);

    if host.is_empty() {
        None
    } else {
        Some(host.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetrcCredentials {
    login: String,
    password: String,
}

fn parse_netrc_credentials(content: &str, host: &str) -> Option<NetrcCredentials> {
    let tokens = netrc_tokens(content);
    let mut index = 0;
    let mut default_credentials = None;

    while index < tokens.len() {
        match tokens[index].as_str() {
            "machine" if index + 1 < tokens.len() => {
                let machine = &tokens[index + 1];
                index += 2;
                let credentials = parse_netrc_entry(&tokens, &mut index);

                if machine.eq_ignore_ascii_case(host) {
                    return credentials;
                }
            }
            "default" => {
                index += 1;
                default_credentials = parse_netrc_entry(&tokens, &mut index);
            }
            _ => index += 1,
        }
    }

    default_credentials
}

fn parse_netrc_entry(tokens: &[String], index: &mut usize) -> Option<NetrcCredentials> {
    let mut login = None;
    let mut password = None;

    while *index < tokens.len() {
        match tokens[*index].as_str() {
            "machine" | "default" => break,
            "login" if *index + 1 < tokens.len() => {
                login = Some(tokens[*index + 1].clone());
                *index += 2;
            }
            "password" if *index + 1 < tokens.len() => {
                password = Some(tokens[*index + 1].clone());
                *index += 2;
            }
            _ => *index += 1,
        }
    }

    match (login, password) {
        (Some(login), Some(password)) if !login.is_empty() => {
            Some(NetrcCredentials { login, password })
        }
        _ => None,
    }
}

fn netrc_tokens(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escaped = false;
    let mut in_comment = false;

    for character in content.chars() {
        if in_comment {
            if character == '\n' {
                in_comment = false;
            }
            continue;
        }

        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        match character {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '#' if !in_quote => {
                push_netrc_token(&mut tokens, &mut current);
                in_comment = true;
            }
            value if value.is_whitespace() && !in_quote => {
                push_netrc_token(&mut tokens, &mut current);
            }
            value => current.push(value),
        }
    }

    push_netrc_token(&mut tokens, &mut current);
    tokens
}

fn push_netrc_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn status_error(status: u16, retry_statuses: &[u16]) -> Result<String, FetchError> {
    let message = format!("HTTP status {status}");

    if should_retry_status(status, retry_statuses) {
        Err(FetchError::retryable(message))
    } else {
        Err(FetchError::fatal(message))
    }
}

fn should_retry_status(status: u16, retry_statuses: &[u16]) -> bool {
    (500..=599).contains(&status) || retry_statuses.contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_server_errors() {
        assert!(should_retry_status(500, &[]));
        assert!(should_retry_status(503, &[]));
    }

    #[test]
    fn does_not_retry_auth_errors() {
        assert!(!should_retry_status(401, &[]));
        assert!(!should_retry_status(403, &[]));
    }

    #[test]
    fn retries_configured_statuses() {
        assert!(should_retry_status(429, &[429]));
        assert!(!should_retry_status(404, &[429]));
    }

    #[test]
    fn parses_machine_credentials_from_netrc() {
        let content = r#"
            machine example.com login alice password "secret value"
            default login guest password guest-pass
        "#;

        assert_eq!(
            parse_netrc_credentials(content, "example.com"),
            Some(NetrcCredentials {
                login: "alice".to_owned(),
                password: "secret value".to_owned(),
            })
        );
    }

    #[test]
    fn parses_default_credentials_from_netrc() {
        let content = "default login guest password guest-pass";

        assert_eq!(
            parse_netrc_credentials(content, "example.com"),
            Some(NetrcCredentials {
                login: "guest".to_owned(),
                password: "guest-pass".to_owned(),
            })
        );
    }

    #[test]
    fn extracts_host_from_url() {
        assert_eq!(
            host_from_url("https://example.com:8443/logs"),
            Some("example.com".to_owned())
        );
        assert_eq!(
            host_from_url("http://user@example.org/logs"),
            Some("example.org".to_owned())
        );
    }
}
