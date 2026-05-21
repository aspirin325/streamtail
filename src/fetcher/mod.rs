use std::{
    fmt, fs,
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
    auth: Option<AuthConfig>,
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
            auth: None,
        }
    }

    pub fn from_config(config: &Config) -> Result<Self, FetchError> {
        let agent = build_agent(
            config.timeout,
            config.ca_cert_path.as_ref(),
            config.insecure,
        )?;

        Ok(Self {
            agent,
            url: config.url.clone(),
            method: config.method.clone(),
            body: config.body.clone(),
            headers: config.headers.clone(),
            auth: config.auth.clone(),
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

    pub fn with_method_body(mut self, method: impl Into<String>, body: Option<String>) -> Self {
        self.method = method.into();
        self.body = body;
        self
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self) -> Result<String, FetchError> {
        let response = if let Some(body) = &self.body {
            self.request().send_string(body)
        } else {
            self.request().call()
        };

        match response {
            Ok(response) => read_body(response),
            Err(ureq::Error::Status(status, _response)) => status_error(status),
            Err(ureq::Error::Transport(err)) => {
                Err(FetchError::retryable(format!("transport error: {err}")))
            }
        }
    }
}

impl HttpFetcher {
    fn request(&self) -> ureq::Request {
        let mut request = self.agent.request(&self.method, &self.url);

        for header in &self.headers {
            request = request.set(&header.name, &header.value);
        }

        match &self.auth {
            Some(AuthConfig::Basic { username, password }) => {
                let credentials = format!("{username}:{password}");
                let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
                request.set("Authorization", &format!("Basic {encoded}"))
            }
            Some(AuthConfig::BearerToken(token)) => {
                request.set("Authorization", &format!("Bearer {token}"))
            }
            None => request,
        }
    }
}

fn build_agent(
    timeout: Duration,
    ca_cert_path: Option<&PathBuf>,
    insecure: bool,
) -> Result<ureq::Agent, FetchError> {
    let builder = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout_write(timeout);

    if insecure || ca_cert_path.is_some() {
        Ok(builder
            .tls_config(Arc::new(build_tls_config(ca_cert_path, insecure)?))
            .build())
    } else {
        Ok(builder.build())
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

fn status_error(status: u16) -> Result<String, FetchError> {
    let message = format!("HTTP status {status}");

    if should_retry_status(status) {
        Err(FetchError::retryable(message))
    } else {
        Err(FetchError::fatal(message))
    }
}

fn should_retry_status(status: u16) -> bool {
    (500..=599).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_server_errors() {
        assert!(should_retry_status(500));
        assert!(should_retry_status(503));
    }

    #[test]
    fn does_not_retry_auth_errors() {
        assert!(!should_retry_status(401));
        assert!(!should_retry_status(403));
    }
}
