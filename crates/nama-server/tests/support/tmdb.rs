//! The fake TMDB a test turns on, and the titles its recorded responses hold, by TMDB id.

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_server::plugins::Registry;

/// Alien, a Movie.
pub(crate) const ALIEN: i64 = 348;
/// Heat, a Movie.
pub(crate) const HEAT: i64 = 949;
/// Dune, a Movie.
pub(crate) const DUNE: i64 = 438_631;
/// Dark, a Show whose Season 1 has three aired Episodes and Season 2 one.
pub(crate) const DARK: i64 = 70_523;
/// Dark's TVDB id, as the fake TMDB gives it.
pub(crate) const DARK_TVDB: i64 = 334_824;
/// The Office (US), a Show.
pub(crate) const THE_OFFICE: i64 = 2316;

/// Starts a fake TMDB and enables the TMDB Plugin in `plugins` against it, with the fake's API
/// key. The fake serves until dropped.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn enable(plugins: &Registry) -> FakeTmdb {
    let fake = FakeTmdb::start().await;
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text(API_KEY.to_owned()),
    )]));
    plugins
        .enable_with_settings(Arc::new(Tmdb::with_endpoints(fake.endpoints())), settings)
        .unwrap();
    fake
}
