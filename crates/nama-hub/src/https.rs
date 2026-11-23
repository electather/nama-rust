//! The HTTPS client the Hub calls outside services with, such as mail and the DNS provider.

use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore, crypto::ring};

/// A client builder trusting Mozilla's roots through ring, as sqlx's TLS does: the Hub's image has
/// no system roots.
pub(crate) fn client() -> Result<reqwest::ClientBuilder, rustls::Error> {
    let tls = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        ))
        .with_no_client_auth();
    Ok(reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .user_agent(concat!("nama-hub/", env!("CARGO_PKG_VERSION"))))
}
