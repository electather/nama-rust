//! The logos Plugins' Manifests give, kept in the [`ImageStore`] as the Server cleaned them and
//! served by URL (ADR 0037). A Plugin's Custom Image, which stands in for its logo, is
//! `CustomImages`'.

use std::{io, path::Path};

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use nama_plugin_api::PluginId;

use crate::images::{ImageStore, SVG, version_of};

/// Where the store keeps them, under the data directory, one folder per Plugin.
const DIR: &str = "plugin-logos";

/// The Manifests' logos.
#[derive(Debug, Clone)]
pub(crate) struct PluginLogos {
    store: ImageStore,
}

impl PluginLogos {
    /// The logos stored in `data_dir`.
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            store: ImageStore::new(data_dir),
        }
    }

    /// Stores `logo`, the Plugin `id`'s Manifest's logo as the Server cleaned it, if it isn't, in
    /// place of any logo stored for the Plugin before, and returns where clients fetch it.
    // ponytail: the id goes into the URL as it is, so one holding `/`, `?` or `#` gets a URL that
    // finds no logo; percent-encode it if Plugin ids ever get such characters.
    pub(crate) async fn stored_url(&self, id: &PluginId, logo: &str) -> io::Result<String> {
        let version = version_of(logo.as_bytes());
        let key = [DIR, id.as_str(), &version];
        if !self.store.contains(&key).await? {
            self.store.write(&key, logo.as_bytes()).await?;
            self.delete_all_but(id, &version).await?;
        }
        Ok(format!("/media/{DIR}/{id}/{version}"))
    }

    /// Deletes every logo stored for the Plugin `id` but the one at `version`.
    async fn delete_all_but(&self, id: &PluginId, version: &str) -> io::Result<()> {
        for key in self.store.list(DIR).await? {
            if let [_, plugin, old] = key.as_slice()
                && plugin == id.as_str()
                && old != version
            {
                self.store.delete(&[DIR, plugin, old]).await?;
            }
        }
        Ok(())
    }
}

/// Serves the logos under `/media/plugin-logos/<id>/<version>`. Who may fetch one is up to the
/// layers the router is put behind.
pub(crate) fn router(logos: PluginLogos) -> Router {
    Router::new()
        .route("/media/plugin-logos/{id}/{version}", get(serve))
        .with_state(logos)
}

async fn serve(
    State(logos): State<PluginLogos>,
    UrlPath((id, version)): UrlPath<(String, String)>,
    request: Request,
) -> Response {
    match logos.store.serve(&[DIR, &id, &version], SVG, request).await {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(plugin = id, "could not serve a Plugin's logo: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
