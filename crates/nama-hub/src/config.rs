//! Hub configuration, from `NAMA_HUB_*` environment variables only: a Hub is deployed, not edited
//! by hand, so it has no config file.

use std::{fmt, net::SocketAddr};

use figment::{Figment, providers::Env};
use nama_forwarded::IpRange;
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
    /// The Resend API key the Hub sends mail with, such as Sign-in Links. Without one it sends
    /// none, so it doesn't offer Sign-in Links.
    pub resend_api_key: Option<String>,
    /// The address mail comes from, such as `Nama Hub <hub@example.com>`, on a domain verified
    /// with Resend. Required with `resend_api_key`.
    pub mail_from: Option<String>,
    /// The Hub's address as people reach it, such as `https://hub.example.com`, which links in
    /// its mail lead to. Required with `resend_api_key`.
    pub public_url: Option<String>,
}

impl Config {
    /// Reads the config from the environment.
    ///
    /// # Errors
    ///
    /// If `NAMA_HUB_DATABASE_URL` is unset, a variable doesn't parse, or
    /// `NAMA_HUB_RESEND_API_KEY` is set without `NAMA_HUB_MAIL_FROM` or `NAMA_HUB_PUBLIC_URL`.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let mut config: Self = Figment::from(Env::prefixed(ENV_PREFIX)).extract()?;
        // A deploy that blanks the variable means no mail, not a key Resend would refuse.
        config.resend_api_key = config.resend_api_key.filter(|key| !key.is_empty());
        match config.missing_for_mail() {
            Some(missing) => Err(Box::new(figment::Error::from(format!(
                "{ENV_PREFIX}RESEND_API_KEY is set, so {ENV_PREFIX}{missing} must be too"
            )))),
            None => Ok(config),
        }
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

fn default_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 7981))
}

// The URL may hold the database's password, and the API key is a secret, so neither is printed.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("listen", &self.listen)
            .field("trusted_proxies", &self.trusted_proxies)
            .field("mail_from", &self.mail_from)
            .field("public_url", &self.public_url)
            .finish_non_exhaustive()
    }
}
