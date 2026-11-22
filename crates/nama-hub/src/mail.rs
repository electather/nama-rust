//! Mail the Hub sends, such as Sign-in Links: a true external, so a trait with the Resend adapter
//! here and a fake in the tests (ADR 0099).

use std::{fmt, future::Future, pin::Pin, sync::Arc, time::Duration};

use reqwest::StatusCode;
use rustls::{ClientConfig, RootCertStore, crypto::ring};
use serde::Serialize;

/// Where the Hub sends mail, and the address the links in it lead to.
#[derive(Clone)]
pub struct Mail {
    /// What sends it.
    pub mailer: Arc<dyn Mailer>,
    /// The Hub's address as people reach it, such as `https://hub.example.com`: never a request's
    /// `Host` header, which whoever sends the request picks.
    pub public_url: String,
}

impl fmt::Debug for Mail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mail")
            .field("public_url", &self.public_url)
            .finish_non_exhaustive()
    }
}

/// A plain-text email to one address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Email {
    /// The address it goes to.
    pub to: String,
    /// Its subject line.
    pub subject: String,
    /// Its body, as plain text.
    pub text: String,
}

/// An email on its way.
pub type Sending<'a> = Pin<Box<dyn Future<Output = Result<(), MailError>> + Send + 'a>>;

/// Something that sends email.
pub trait Mailer: Send + Sync {
    /// Sends `email` once: a failure is the caller's to log, never retried.
    fn send(&self, email: Email) -> Sending<'_>;
}

/// Why an email wasn't sent.
#[derive(Debug, thiserror::Error)]
pub enum MailError {
    /// The mail service couldn't be reached, or didn't answer in time.
    #[error("could not reach the mail service: {0}")]
    Request(#[from] reqwest::Error),
    /// The mail service answered with an error.
    #[error("the mail service refused the email with {status}: {body}")]
    Refused {
        /// Its HTTP status.
        status: StatusCode,
        /// What it said.
        body: String,
    },
    /// The HTTPS client couldn't be set up.
    #[error("could not set up TLS: {0}")]
    Tls(#[from] rustls::Error),
}

/// Sends email through Resend's REST API.
pub struct Resend {
    client: reqwest::Client,
    /// [`RESEND_URL`], or a stand-in's in tests.
    url: String,
    api_key: String,
    from: String,
}

/// Resend's endpoint for sending one email.
const RESEND_URL: &str = "https://api.resend.com/emails";

/// How long one send may take before it counts as failed.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

impl Resend {
    /// Sends with the API key `api_key`, from the address `from`, such as `Nama Hub
    /// <hub@example.com>`, which must be on a domain verified with Resend.
    ///
    /// # Errors
    ///
    /// If the HTTPS client can't be set up.
    pub fn new(api_key: String, from: String) -> Result<Self, MailError> {
        // ring, as sqlx's TLS uses, and Mozilla's roots: the Hub's image has no system ones.
        let tls = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            ))
            .with_no_client_auth();
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .user_agent(concat!("nama-hub/", env!("CARGO_PKG_VERSION")))
            .timeout(SEND_TIMEOUT)
            .build()?;
        Ok(Self {
            client,
            url: RESEND_URL.to_owned(),
            api_key,
            from,
        })
    }
}

/// The body Resend's send endpoint takes.
#[derive(Serialize)]
struct ResendEmail<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: &'a str,
}

impl Mailer for Resend {
    fn send(&self, email: Email) -> Sending<'_> {
        Box::pin(async move {
            let response = self
                .client
                .post(&self.url)
                .bearer_auth(&self.api_key)
                .json(&ResendEmail {
                    from: &self.from,
                    to: [&email.to],
                    subject: &email.subject,
                    text: &email.text,
                })
                .send()
                .await?;
            let status = response.status();
            if status.is_success() {
                return Ok(());
            }
            let body = response.text().await.unwrap_or_default();
            Err(MailError::Refused { status, body })
        })
    }
}

// The API key is a secret, so it is never printed.
impl fmt::Debug for Resend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resend")
            .field("from", &self.from)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "mail_tests.rs"]
mod tests;
