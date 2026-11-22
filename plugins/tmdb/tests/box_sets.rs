//! A Movie's Box Set, from TMDB's collections, against recorded TMDB responses served locally.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers fail their test by panicking"
)]

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    Plugin, Registrar, Registration,
    contract::{BoxSet, TitleAsked, TitleId},
    metadata::TitleSource,
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};

/// The title source the Plugin registers with "List Box Set Movies the Server lacks" as given,
/// or as its default when `None`, talking to `fake`.
fn provider(fake: &FakeTmdb, box_set_movies: Option<bool>) -> Arc<dyn TitleSource> {
    let plugin = Tmdb::with_endpoints(fake.endpoints());
    let mut settings =
        BTreeMap::from([("api_key".to_owned(), SettingValue::Text(API_KEY.to_owned()))]);
    if let Some(listed) = box_set_movies {
        settings.insert("box_set_movies".to_owned(), SettingValue::Bool(listed));
    }
    let settings = Settings::new(settings)
        .resolve(&plugin.manifest().settings)
        .unwrap();
    let mut registrar = Registrar::with_settings(settings);
    plugin.register(&mut registrar).unwrap();
    let registrations = registrar.into_registrations();
    registrations
        .iter()
        .find_map(Registration::provider)
        .expect("a title source")
}

fn tmdb(id: i64) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..Default::default()
    }
}

/// The Box Set the Movie `id` answers with.
async fn box_set_of(source: &dyn TitleSource, id: i64) -> Option<BoxSet> {
    let asked = TitleAsked {
        id: tmdb(id).into(),
        ..Default::default()
    };
    let answer = source.movie(&asked).await.unwrap();
    answer.movie.into_option().unwrap().box_set.into_option()
}

#[test]
fn listing_box_set_movies_is_an_admin_setting_on_by_default() {
    let settings = Tmdb::new().manifest().settings;
    let field = (settings.fields.iter())
        .find(|field| field.key == "box_set_movies")
        .unwrap();

    assert_eq!(field.label, "List Box Set Movies the Server lacks");
    assert_eq!(field.default, Some(SettingValue::Bool(true)));
}

#[tokio::test]
async fn a_movie_has_its_box_set_with_its_overview_and_movies_in_release_order() {
    let fake = FakeTmdb::start().await;

    let box_set = box_set_of(&*provider(&fake, None), 348).await.unwrap();

    assert_eq!(box_set.id.into_option(), Some(tmdb(8091)));
    assert_eq!(box_set.name, "Alien Collection");
    assert!(box_set.overview.unwrap().starts_with("A science fiction"));
    assert_eq!(
        box_set.poster.as_deref(),
        Some("/iVzIeC3PbG9BtDAudpwSNOKmOJa.jpg")
    );
    assert_eq!(
        box_set.backdrop.as_deref(),
        Some("/kB0Y3uGe9ohJa59Lk8UO9cUOxGM.jpg")
    );
    assert_eq!(box_set.movies, [tmdb(348), tmdb(679), tmdb(999_999_901)]);
}

#[tokio::test]
async fn with_box_set_movies_off_a_box_set_has_no_movies_and_no_collection_is_asked() {
    let fake = FakeTmdb::start().await;

    let box_set = box_set_of(&*provider(&fake, Some(false)), 348)
        .await
        .unwrap();

    assert_eq!(box_set.name, "Alien Collection");
    assert_eq!(box_set.overview, None);
    assert_eq!(box_set.movies, []);
    let asked = fake.requests();
    assert!(
        !asked.iter().any(|request| request.contains("/collection/")),
        "{asked:?}"
    );
}

#[tokio::test]
async fn a_box_sets_collection_is_fetched_once_for_its_movies_fetched_together() {
    let fake = FakeTmdb::start().await;
    let source = provider(&fake, None);

    box_set_of(&*source, 348).await;
    box_set_of(&*source, 348).await;

    let asked = fake.requests();
    let collections = asked
        .iter()
        .filter(|request| request.contains("/collection/"));
    assert_eq!(collections.count(), 1, "{asked:?}");
}

#[tokio::test]
async fn a_movie_in_no_collection_has_no_box_set() {
    let fake = FakeTmdb::start().await;

    assert_eq!(box_set_of(&*provider(&fake, None), 949).await, None);
}
