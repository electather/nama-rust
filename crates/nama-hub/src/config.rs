//! Hub configuration, from `NAMA_HUB_*` environment variables only: a Hub is deployed, not edited
//! by hand, so it has no config file.

use std::{fmt, net::SocketAddr};

use figment::{Figment, providers::Env};
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

// The URL may hold the database's password, so it is never printed.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("listen", &self.listen)
            .finish_non_exhaustive()
    }
}
