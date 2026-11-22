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
    /// The Ed25519 private keys the Hub signs tokens for Servers with, as PKCS#8 PEM blocks one
    /// after another: the first signs, and all are published (ADR 0098). Required to serve.
    #[serde(default)]
    pub signing_keys: String,
}

impl Config {
    /// Reads the config from the environment.
    ///
    /// # Errors
    ///
    /// If `NAMA_HUB_DATABASE_URL` is unset, or a variable doesn't parse.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Ok(Figment::from(Env::prefixed(ENV_PREFIX)).extract()?)
    }
}

fn default_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 7981))
}

// The URL may hold the database's password, and the keys are secret, so neither is printed.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("listen", &self.listen)
            .field("trusted_proxies", &self.trusted_proxies)
            .finish_non_exhaustive()
    }
}
