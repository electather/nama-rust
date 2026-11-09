//! The TMDB Plugin's metadata Provider, against recorded TMDB responses served locally.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers fail their test by panicking"
)]

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    Plugin, Registrar,
    metadata::{
        Candidate, ExternalId, ExternalIds, MetadataError, MetadataProvider, SearchQuery, TitleKind,
    },
    settings::{SettingKind, SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};

/// The Provider the Plugin registers when enabled with `api_key`, talking to `fake`.
fn provider(fake: &FakeTmdb, api_key: &str) -> Arc<dyn MetadataProvider> {
    let plugin = Tmdb::with_endpoints(fake.endpoints());
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text(api_key.to_owned()),
    )]))
    .resolve(&plugin.manifest().settings)
    .unwrap();
    let mut registrar = Registrar::with_settings(settings);
    plugin.register(&mut registrar);
    let registrations = registrar.into_registrations();
    assert_eq!(registrations.len(), 1);
    registrations[0]
        .provider::<dyn MetadataProvider>()
        .expect("a MetadataProvider")
}

fn query(kind: TitleKind, title: &str, year: Option<i32>) -> SearchQuery {
    SearchQuery {
        kind,
        title: title.to_owned(),
        year,
    }
}

#[test]
fn the_api_key_is_a_required_secret_setting() {
    let schema = Tmdb::new().manifest().settings;

    let api_key = schema
        .fields
        .iter()
        .find(|field| field.key == "api_key")
        .unwrap();
    assert_eq!(api_key.kind, SettingKind::Secret);
    assert!(api_key.required);
}

#[tokio::test]
async fn searching_movies_finds_candidates_with_their_years() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let found = tmdb
        .search(&query(TitleKind::Movie, "Dune", Some(2021)))
        .await
        .unwrap();

    let dune = |id, year, poster: &str, vote_count| Candidate {
        id: ExternalId::Tmdb(id),
        title: "Dune".to_owned(),
        original_title: Some("Dune".to_owned()),
        year: Some(year),
        poster: Some(poster.to_owned()),
        vote_count,
    };
    assert_eq!(found.len(), 3);
    assert_eq!(
        found[0],
        dune(438_631, 2021, "/d5NXSklXo0qyIYkgV94XAgMIckC.jpg", 12_634)
    );
    assert_eq!(
        found[2],
        dune(841, 1984, "/a3nDwAnKAl0jsSmsGaen2Ndqidy.jpg", 2_770)
    );
    let requested = fake.requests();
    assert!(
        requested[0].starts_with("/3/search/movie?")
            && requested[0].contains("query=Dune")
            && requested[0].contains("year=2021")
            && requested[0].contains(&format!("api_key={API_KEY}")),
        "{requested:?}"
    );
}

#[tokio::test]
async fn searching_shows_uses_their_first_air_year() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let found = tmdb
        .search(&query(TitleKind::Show, "Dark", Some(2017)))
        .await
        .unwrap();

    assert_eq!(
        found,
        [Candidate {
            id: ExternalId::Tmdb(70523),
            title: "Dark".to_owned(),
            original_title: Some("Dark".to_owned()),
            year: Some(2017),
            poster: Some("/apbrbWs8M9lyOpJYU5WXrpFbk1Z.jpg".to_owned()),
            vote_count: 6_900,
        }]
    );
}

#[tokio::test]
async fn a_search_that_finds_nothing_is_empty() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let found = tmdb
        .search(&query(TitleKind::Movie, "Home Movies", Some(2019)))
        .await
        .unwrap();

    assert_eq!(found, []);
}

#[tokio::test]
async fn a_movie_has_its_details_imdb_id_and_artwork() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let movie = tmdb.movie(&ExternalId::Tmdb(348)).await.unwrap();

    assert_eq!(
        movie.ids,
        ExternalIds {
            tmdb: Some(348),
            imdb: Some("tt0078748".to_owned()),
            tvdb: None,
        }
    );
    assert_eq!(movie.title, "Alien");
    assert_eq!(movie.release_date.as_deref(), Some("1979-05-25"));
    assert_eq!(movie.runtime_minutes, Some(117));
    assert!(movie.overview.unwrap().starts_with("During its return"));
    assert_eq!(
        movie.poster.as_deref(),
        Some("/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
    );
    assert_eq!(
        movie.backdrop.as_deref(),
        Some("/AmR3JG1VQVxU8TfAvljUhfSFUOx.jpg")
    );
}

#[tokio::test]
async fn a_show_has_its_tvdb_and_imdb_ids_and_seasons() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let show = tmdb.show(&ExternalId::Tmdb(70523)).await.unwrap();

    assert_eq!(
        show.ids,
        ExternalIds {
            tmdb: Some(70523),
            imdb: Some("tt5753856".to_owned()),
            tvdb: Some(334_824),
        }
    );
    assert_eq!(show.title, "Dark");
    assert_eq!(show.first_air_date.as_deref(), Some("2017-12-01"));
    assert_eq!(show.seasons, [1, 2, 3]);
}

#[tokio::test]
async fn a_season_has_its_episodes() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let season = tmdb.season(&ExternalId::Tmdb(70523), 1).await.unwrap();

    assert_eq!(season.number, 1);
    assert_eq!(season.title, "Season 1");
    assert_eq!(season.ids.tvdb, Some(727_934));
    let episodes: Vec<(u32, u32, &str, Option<i64>)> = season
        .episodes
        .iter()
        .map(|e| (e.season, e.number, e.title.as_str(), e.ids.tmdb))
        .collect();
    assert_eq!(
        episodes,
        [
            (1, 1, "Secrets", Some(1_388_538)),
            (1, 2, "Lies", Some(1_388_539)),
            (1, 3, "Past and Present", Some(1_388_540)),
        ]
    );
    assert_eq!(season.episodes[0].runtime_minutes, Some(52));
}

#[tokio::test]
async fn an_episode_has_its_own_ids() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let episode = tmdb.episode(&ExternalId::Tmdb(70523), 1, 1).await.unwrap();

    assert_eq!(
        episode.ids,
        ExternalIds {
            tmdb: Some(1_388_538),
            imdb: Some("tt5753730".to_owned()),
            tvdb: Some(6_288_618),
        }
    );
    assert_eq!(episode.title, "Secrets");
    assert_eq!(
        episode.still.as_deref(),
        Some("/gNnJBvHy3VBkGrsl9ffpkf6ucP3.jpg")
    );
}

#[tokio::test]
async fn an_unknown_title_is_not_found() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let missing = tmdb.movie(&ExternalId::Tmdb(1)).await;

    assert!(
        matches!(missing, Err(MetadataError::NotFound)),
        "{missing:?}"
    );
}

#[tokio::test]
async fn only_tmdb_ids_can_be_looked_up() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);
    let imdb = ExternalId::Imdb("tt0078748".to_owned());

    let result = tmdb.movie(&imdb).await;

    assert!(
        matches!(result, Err(MetadataError::UnsupportedId(ref id)) if *id == imdb),
        "{result:?}"
    );
    assert_eq!(fake.requests(), Vec::<String>::new());
}

#[tokio::test]
async fn a_rejected_api_key_is_a_failure_not_a_missing_title() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, "wrong");

    let result = tmdb.movie(&ExternalId::Tmdb(348)).await;

    let Err(MetadataError::Failed(error)) = result else {
        panic!("expected a failure, got {result:?}");
    };
    assert!(error.to_string().contains("API key"), "{error}");
}

#[tokio::test]
async fn an_image_is_fetched_by_the_path_tmdb_gave() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let image = tmdb
        .image("/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
        .await
        .unwrap();

    assert_eq!(image.content_type, "image/jpeg");
    assert!(image.bytes.starts_with(b"\xff\xd8"));
    assert_eq!(
        fake.requests(),
        ["/t/p/original/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg"]
    );
}
