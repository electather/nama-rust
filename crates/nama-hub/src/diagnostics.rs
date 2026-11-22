//! Diagnostics and Usage Analytics that Linked Servers share with Nama's developers (ADR 0054):
//! each Server may send only so much a minute, and what it sends goes on unchanged to the backend
//! the Hub's config names. The backend is a true external, so a trait with the OTLP adapter here
//! and a fake in the tests (ADR 0099).

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::body::Bytes;
use reqwest::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, InvalidHeaderName},
};
use rustls::{ClientConfig, RootCertStore, crypto::ring};

use crate::links::LinkedServerId;

/// How many bytes one Linked Server may share in [`WINDOW`]: a busy Server's batches, a few
/// hundred spans every 5 seconds, fit with room to spare, while one gone wrong can't run up the
/// backend's bill.
const BYTES_PER_WINDOW: usize = 4 * 1024 * 1024;

const WINDOW: Duration = Duration::from_mins(1);

/// How long the backend has to take a batch before it fails, so a stuck one holds no request.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(10);

/// Which OTLP signal a batch holds, as its OTLP/HTTP path names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Spans.
    Traces,
    /// Log records: a Server's allow-listed log lines and its Usage Analytics.
    Logs,
}

impl Signal {
    /// Its path under an OTLP/HTTP endpoint.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Traces => "v1/traces",
            Self::Logs => "v1/logs",
        }
    }
}

/// A batch on its way to the backend.
pub type Forwarding<'a> = Pin<Box<dyn Future<Output = Result<(), BackendError>> + Send + 'a>>;

/// Where shared Diagnostics go, such as an OpenTelemetry Collector or a vendor's OTLP endpoint.
pub trait Backend: Send + Sync {
    /// Forwards `batch`, an OTLP/HTTP protobuf export request of `signal`, unchanged and once: a
    /// failure is the caller's to log, never retried.
    fn forward(&self, signal: Signal, batch: Bytes) -> Forwarding<'_>;
}

/// Why the backend didn't take a batch.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    /// The backend couldn't be reached, or didn't answer in time.
    #[error("could not reach the Diagnostics backend: {0}")]
    Request(#[from] reqwest::Error),
    /// The backend answered with an error.
    #[error("the Diagnostics backend refused a batch with {0}")]
    Refused(StatusCode),
}

/// Why an [`Otlp`] backend couldn't be set up.
#[derive(Debug, thiserror::Error)]
pub enum OtlpError {
    /// A header isn't `key=value`, or its key or value can't be sent.
    #[error("the Diagnostics backend's headers must be key=value pairs, comma-separated")]
    Header,
    /// The HTTPS client couldn't be set up.
    #[error("could not set up the Diagnostics backend's HTTPS client: {0}")]
    Client(#[from] reqwest::Error),
    /// TLS couldn't be set up.
    #[error("could not set up TLS: {0}")]
    Tls(#[from] rustls::Error),
}

impl From<InvalidHeaderName> for OtlpError {
    fn from(_: InvalidHeaderName) -> Self {
        Self::Header
    }
}

/// A backend taking OTLP/HTTP.
pub struct Otlp {
    client: reqwest::Client,
    /// Without a trailing slash.
    endpoint: String,
}

impl Otlp {
    /// The backend at `endpoint`, such as `https://otlp.example.com/otlp`, under which each signal
    /// posts to its own path, as `OTEL_EXPORTER_OTLP_ENDPOINT` is read. Every batch carries
    /// `headers`, written as `OTEL_EXPORTER_OTLP_HEADERS` is, such as `authorization=Basic abc`.
    ///
    /// # Errors
    ///
    /// [`OtlpError::Header`] for a header that isn't `key=value`, or if the HTTPS client can't be
    /// set up.
    pub fn new(endpoint: &str, headers: &str) -> Result<Self, OtlpError> {
        // ring and Mozilla's roots, as `mail.rs`'s client: the Hub's image has no system ones.
        let tls = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            ))
            .with_no_client_auth();
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .user_agent(concat!("nama-hub/", env!("CARGO_PKG_VERSION")))
            .timeout(FORWARD_TIMEOUT)
            .default_headers(parse_headers(headers)?)
            .build()?;
        Ok(Self {
            client,
            endpoint: endpoint.trim_end_matches('/').to_owned(),
        })
    }
}

/// `headers`, written as `OTEL_EXPORTER_OTLP_HEADERS` is: `key=value` pairs, comma-separated.
fn parse_headers(headers: &str) -> Result<HeaderMap, OtlpError> {
    let mut parsed = HeaderMap::new();
    for header in headers
        .split(',')
        .filter(|header| !header.trim().is_empty())
    {
        let (key, value) = header.split_once('=').ok_or(OtlpError::Header)?;
        let mut value =
            HeaderValue::from_str(value.trim()).map_err(|_invalid| OtlpError::Header)?;
        // An API key, never printed.
        value.set_sensitive(true);
        parsed.insert(HeaderName::try_from(key.trim())?, value);
    }
    Ok(parsed)
}

impl Backend for Otlp {
    fn forward(&self, signal: Signal, batch: Bytes) -> Forwarding<'_> {
        Box::pin(async move {
            let response = self
                .client
                .post(format!("{}/{}", self.endpoint, signal.path()))
                .header(CONTENT_TYPE, "application/x-protobuf")
                .body(batch)
                .send()
                .await?;
            let status = response.status();
            if status.is_success() {
                Ok(())
            } else {
                Err(BackendError::Refused(status))
            }
        })
    }
}

// The headers hold an API key, so they are never printed.
impl fmt::Debug for Otlp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Otlp")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

/// Why a Server's batch wasn't taken.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NotTaken {
    /// The Server shared more than [`BYTES_PER_WINDOW`] in the current [`WINDOW`].
    #[error("the Server shared more than it may in a minute")]
    TooMuch,
    /// The backend didn't take it.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// What Linked Servers share, on its way to the backend. Clones share the counts.
#[derive(Clone)]
pub(crate) struct Shared {
    /// `None` while the Hub's config names no backend: what Servers share is dropped.
    backend: Option<Arc<dyn Backend>>,
    /// When each Server's current window started, and how many bytes it has shared in it.
    sent: Arc<Mutex<HashMap<LinkedServerId, (Instant, usize)>>>,
}

impl Shared {
    /// Forwards to `backend`, or drops everything without one.
    pub(crate) fn new(backend: Option<Arc<dyn Backend>>) -> Self {
        Self {
            backend,
            sent: Arc::default(),
        }
    }

    /// Forwards `batch`, of `signal`, from `server` to the backend, unless `server` has shared too
    /// much lately, as of `now`.
    pub(crate) async fn take(
        &self,
        server: LinkedServerId,
        signal: Signal,
        batch: Bytes,
        now: Instant,
    ) -> Result<(), NotTaken> {
        if !self.admit(server, batch.len(), now) {
            return Err(NotTaken::TooMuch);
        }
        if let Some(backend) = &self.backend {
            backend.forward(signal, batch).await?;
        }
        Ok(())
    }

    /// Counts `bytes` from `server` at `now`, and whether they are within its limit.
    fn admit(&self, server: LinkedServerId, bytes: usize, now: Instant) -> bool {
        // Every change leaves the counts consistent, so a panic elsewhere doesn't spoil them.
        let mut sent = self.sent.lock().unwrap_or_else(PoisonError::into_inner);
        sent.retain(|_, (started, _)| now.saturating_duration_since(*started) < WINDOW);
        let (_, count) = sent.entry(server).or_insert((now, 0));
        *count = count.saturating_add(bytes);
        *count <= BYTES_PER_WINDOW
    }
}

#[cfg(test)]
#[path = "diagnostics/diagnostics_tests.rs"]
mod tests;
