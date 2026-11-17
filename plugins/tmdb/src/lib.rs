//! A Built-in Plugin that provides metadata from [TMDB](https://www.themoviedb.org), the only
//! title source built into the Server.
//!
//! Enabled with an API key in its settings, it registers a title source,
//! [`nama_plugin_api::metadata::TitleSource`], that searches TMDB and fetches
//! Movie, Show, Season and Episode details and images by TMDB id, along with the IMDb and TVDB ids
//! TMDB knows. Without an API key the Kernel doesn't enable it.
//!
//! This product uses the TMDB API but is not endorsed or certified by TMDB.

mod client;
#[cfg(feature = "testing")]
pub mod testing;
mod wire;

use std::sync::Arc;

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    metadata::{SourceManifest, TMDB, TitleSource},
    settings::{SettingField, SettingKind, SettingValue, SettingsSchema},
};

use crate::client::Client;

const API_KEY: &str = "api_key";

const LANGUAGE: &str = "language";

const DEFAULT_LANGUAGE: &str = "en-US";

/// The TMDB Built-in Plugin, providing the [`TitleSource`] that Matching uses.
#[derive(Debug, Clone, Default)]
pub struct Tmdb {
    endpoints: Endpoints,
}

/// Where TMDB's API and images are served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The base URL of the v3 API, without a trailing `/`.
    pub api: String,
    /// The base URL images are served under, without a trailing `/` or a size.
    pub images: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            api: "https://api.themoviedb.org/3".to_owned(),
            images: "https://image.tmdb.org/t/p".to_owned(),
        }
    }
}

impl Tmdb {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("tmdb");

    /// The Plugin, talking to TMDB itself.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The Plugin, talking to TMDB at `endpoints` instead, such as a stand-in for tests.
    #[must_use]
    pub const fn with_endpoints(endpoints: Endpoints) -> Self {
        Self { endpoints }
    }
}

impl Plugin for Tmdb {
    fn manifest(&self) -> Manifest {
        let settings = SettingsSchema {
            fields: vec![
                SettingField {
                    key: API_KEY.to_owned(),
                    label: "API key".to_owned(),
                    description:
                        "Your TMDB API key (v3), from themoviedb.org under Settings → API."
                            .to_owned(),
                    kind: SettingKind::Secret,
                    required: true,
                    default: None,
                },
                SettingField {
                    key: LANGUAGE.to_owned(),
                    label: "Language".to_owned(),
                    description: "The language titles and overviews are fetched in, as an IETF tag such as en-US or de-DE. Its country, such \
                                  as US, picks the Content Rating."
                        .to_owned(),
                    kind: SettingKind::Text,
                    required: false,
                    default: Some(SettingValue::Text(DEFAULT_LANGUAGE.to_owned())),
                },
            ],
        };
        Manifest {
            id: Self::ID,
            name: "TMDB".to_owned(),
            description: "Finds the details and artwork of Movies and Shows on The Movie Database."
                .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            settings,
            personal_settings: SettingsSchema::default(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: Vec::new(),
            tick_every: None,
            title_source: Some(SourceManifest {
                namespace: TMDB.to_owned(),
                looks_up: client::looks_up(),
                searches: true,
            }),
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let settings = registrar.settings();
        // The Kernel only enables the Plugin with an API key.
        let api_key = settings
            .text(API_KEY)
            .ok_or("the tmdb plugin needs an API key")?;
        let language = settings.text(LANGUAGE).unwrap_or(DEFAULT_LANGUAGE);
        let client = Client::new(self.endpoints.clone(), api_key, language)
            .map_err(|err| format!("could not set up HTTPS for TMDB: {err}"))?;
        let source: Arc<dyn TitleSource> = Arc::new(client);
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, source);
        Ok(())
    }
}
