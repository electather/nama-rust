//! Hub configuration, from `NAMA_HUB_*` environment variables only: a Hub is deployed, not edited
//! by hand, so it has no config file.

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
};

use figment::{Figment, providers::Env};
use nama_forwarded::IpRange;
use reqwest::Url;
use serde::Deserialize;

/// Prefix of the environment variables the config is read from, e.g. `NAMA_HUB_LISTEN`.
const ENV_PREFIX: &str = "NAMA_HUB_";

/// Everything a Hub needs to know before it starts.
#[derive(Deserialize)]
pub struct Config {
    /// The PostgreSQL database the Hub keeps everything in, as a `postgres://` URL. Required.
    pub database_url: String,
    /// Address the Hub listens on.
    #[serde(default = "default_listen")]
    pub listen: SocketAddr,
    /// The reverse proxies whose `X-Forwarded-For` and `X-Forwarded-Proto` the Hub believes, as IP
    /// addresses or CIDR ranges, such as `["10.0.0.0/8"]` (ADR 0018). None by default.
    #[serde(default)]
    pub trusted_proxies: Vec<IpRange>,
    /// The Ed25519 private keys the Hub signs tokens for Servers with, as PKCS#8 PEM blocks one
    /// after another: the first signs, and all are published (ADR 0098). Required to serve.
    #[serde(default)]
    pub signing_keys: String,
    /// The Resend API key the Hub sends mail with, such as Sign-in Links. Without one it sends
    /// none, so it doesn't offer Sign-in Links.
    pub resend_api_key: Option<String>,
    /// Writes the mail to the Hub's log instead of sending it, for developing without Resend.
    /// Only with a `public_url` on this machine, and never with `resend_api_key`.
    #[serde(default)]
    pub log_mail: bool,
    /// The address mail comes from, such as `Nama Hub <hub@example.com>`, on a domain verified
    /// with Resend. Required with `resend_api_key`.
    pub mail_from: Option<String>,
    /// The Hub's address as people reach it, such as `https://hub.example.com`, which links in
    /// its mail lead to. Required with `resend_api_key` or `log_mail`.
    pub public_url: Option<String>,
    /// The OTLP/HTTP endpoint the Diagnostics and Usage Analytics Servers share go to, such as
    /// `https://otlp.example.com/otlp` (ADR 0054). Without one, what they share is dropped.
    pub diagnostics_endpoint: Option<String>,
    /// The headers sent with each batch to `diagnostics_endpoint`, written as
    /// `OTEL_EXPORTER_OTLP_HEADERS` is, such as `authorization=Basic abc`. None by default.
    #[serde(default)]
    pub diagnostics_headers: String,
}

impl Config {
    /// Reads the config from the environment.
    ///
    /// # Errors
    ///
    /// If `NAMA_HUB_DATABASE_URL` is unset, a variable doesn't parse,
    /// `NAMA_HUB_RESEND_API_KEY` is set without `NAMA_HUB_MAIL_FROM` or `NAMA_HUB_PUBLIC_URL`, or
    /// `NAMA_HUB_LOG_MAIL` is set with that key or without a loopback `NAMA_HUB_PUBLIC_URL`.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let mut config: Self = Figment::from(Env::prefixed(ENV_PREFIX)).extract()?;
        // A deploy that blanks the variable means no mail, not a key Resend would refuse.
        config.resend_api_key = config.resend_api_key.filter(|key| !key.is_empty());
        config.diagnostics_endpoint = config.diagnostics_endpoint.filter(|url| !url.is_empty());
        let refusal = if config.log_mail {
            config.log_mail_refusal()
        } else {
            config.missing_for_mail().map(|missing| {
                format!("{ENV_PREFIX}RESEND_API_KEY is set, so {ENV_PREFIX}{missing} must be too")
            })
        };
        match refusal {
            Some(refusal) => Err(Box::new(figment::Error::from(refusal))),
            None => Ok(config),
        }
    }

    /// Why `log_mail` can't be on, if it can't. Links to a loopback address work only on the
    /// developer's own machine, so this keeps a real deploy from logging its mail by mistake.
    fn log_mail_refusal(&self) -> Option<String> {
        if self.resend_api_key.is_some() {
            return Some(format!(
                "{ENV_PREFIX}LOG_MAIL and {ENV_PREFIX}RESEND_API_KEY can't both be set"
            ));
        }
        let Some(url) = &self.public_url else {
            return Some(format!(
                "{ENV_PREFIX}LOG_MAIL is set, so {ENV_PREFIX}PUBLIC_URL must be too"
            ));
        };
        (!is_on_loopback(url)).then(|| {
            format!(
                "{ENV_PREFIX}LOG_MAIL is set, so {ENV_PREFIX}PUBLIC_URL must be on localhost or a \
                 loopback address, not {url}"
            )
        })
    }

    /// The variable mail needs that is unset, if `resend_api_key` is set. The public URL is
    /// never taken from a request's Host header, which whoever sends the request picks.
    fn missing_for_mail(&self) -> Option<&'static str> {
        self.resend_api_key.as_ref()?;
        if self.mail_from.is_none() {
            Some("MAIL_FROM")
        } else if self.public_url.is_none() {
            Some("PUBLIC_URL")
        } else {
            None
        }
    }
}

/// Whether `url`'s host is `localhost` or a loopback IP address.
fn is_on_loopback(url: &str) -> bool {
    match Url::parse(url).as_ref().map(Url::host_str) {
        Ok(Some("localhost")) => true,
        // An IPv6 host comes bracketed, as in `http://[::1]`.
        Ok(Some(host)) => host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        _ => false,
    }
}

fn default_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 7981))
}

// The URL may hold the database's password, and the keys, the API key and the Diagnostics
// backend's headers are secrets, so none is printed.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("listen", &self.listen)
            .field("trusted_proxies", &self.trusted_proxies)
            .field("mail_from", &self.mail_from)
            .field("public_url", &self.public_url)
            .field("log_mail", &self.log_mail)
            .field("diagnostics_endpoint", &self.diagnostics_endpoint)
            .finish_non_exhaustive()
    }
}
