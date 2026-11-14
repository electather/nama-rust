//! The TMDB Plugin's title source, against recorded TMDB responses served locally.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers fail their test by panicking"
)]

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    Plugin, Registrar,
    contract::{
        Candidate, EpisodeAsked, ImageAsked, SearchQuery, SeasonAsked, TitleAsked, TitleId,
        TitleKind,
    },
    metadata::{ExternalId, ExternalIds, IMDB, MetadataError, TMDB, TVDB, TitleSource},
    settings::{SettingKind, SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};

/// The title source the Plugin registers when enabled with `api_key`, talking to `fake`.
fn provider(fake: &FakeTmdb, api_key: &str) -> Arc<dyn TitleSource> {
    let plugin = Tmdb::with_endpoints(fake.endpoints());
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text(api_key.to_owned()),
    )]))
    .resolve(&plugin.manifest().settings)
    .unwrap();
    let mut registrar = Registrar::with_settings(settings);
    plugin.register(&mut registrar).unwrap();
    let registrations = registrar.into_registrations();
    assert_eq!(registrations.len(), 1);
    registrations[0]
        .provider::<dyn TitleSource>()
        .expect("a TitleSource")
}

fn query(kind: TitleKind, title: &str, year: Option<i32>) -> SearchQuery {
    SearchQuery {
        kind: kind.into(),
        title: title.to_owned(),
        year,
        ..Default::default()
    }
}

fn title_id(id: &ExternalId) -> TitleId {
    TitleId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..Default::default()
    }
}

fn asked(id: &ExternalId) -> TitleAsked {
    TitleAsked {
        id: title_id(id).into(),
        ..Default::default()
    }
}

fn season_asked(show: &ExternalId, season: u32) -> SeasonAsked {
    SeasonAsked {
        show: title_id(show).into(),
        season,
        ..Default::default()
    }
}

/// `ids` as a set, to compare without minding their order.
fn ids(ids: &[TitleId]) -> ExternalIds {
    ids.iter()
        .map(|id| ExternalId::new(&id.namespace, id.value.clone()).unwrap())
        .collect()
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

#[test]
fn registering_without_an_api_key_fails_and_registers_nothing() {
    let mut registrar = Registrar::with_settings(Settings::default());

    let err = Tmdb::new().register(&mut registrar).unwrap_err();

    assert!(err.to_string().contains("API key"), "{err}");
    assert!(registrar.into_registrations().is_empty());
}

#[tokio::test]
async fn it_keeps_titles_under_the_tmdb_namespace() {
    let fake = FakeTmdb::start().await;

    assert_eq!(provider(&fake, API_KEY).namespace(), TMDB);
}

#[tokio::test]
async fn searching_movies_finds_candidates_with_their_years() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let found = tmdb
        .search(&query(TitleKind::TITLE_KIND_MOVIE, "Dune", Some(2021)))
        .await
        .unwrap()
        .candidates;

    let dune = |id, year, poster: &str, vote_count| Candidate {
        id: title_id(&ExternalId::tmdb(id)).into(),
        ids: vec![title_id(&ExternalId::tmdb(id))],
        title: "Dune".to_owned(),
        original_title: Some("Dune".to_owned()),
        year: Some(year),
        poster: Some(poster.to_owned()),
        origin_country: Vec::new(),
        vote_count,
        ..Default::default()
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
async fn searching_shows_uses_their_first_air_year_and_has_their_countries() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let found = tmdb
        .search(&query(TitleKind::TITLE_KIND_SHOW, "Dark", Some(2017)))
        .await
        .unwrap()
        .candidates;

    assert_eq!(
        found,
        [Candidate {
            id: title_id(&ExternalId::tmdb(70523)).into(),
            ids: vec![title_id(&ExternalId::tmdb(70523))],
            title: "Dark".to_owned(),
            original_title: Some("Dark".to_owned()),
            year: Some(2017),
            poster: Some("/apbrbWs8M9lyOpJYU5WXrpFbk1Z.jpg".to_owned()),
            origin_country: vec!["DE".to_owned()],
            vote_count: 6_900,
            ..Default::default()
        }]
    );
}

#[tokio::test]
async fn a_search_that_finds_nothing_is_empty() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let found = tmdb
        .search(&query(
            TitleKind::TITLE_KIND_MOVIE,
            "Home Movies",
            Some(2019),
        ))
        .await
        .unwrap()
        .candidates;

    assert_eq!(found, []);
}

#[tokio::test]
async fn a_movie_has_its_details_imdb_id_and_artwork() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let movie = tmdb.movie(&asked(&ExternalId::tmdb(348))).await.unwrap();
    let movie = movie.movie.unwrap();

    assert_eq!(
        ids(&movie.ids),
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")])
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

    let show = tmdb.show(&asked(&ExternalId::tmdb(70523))).await.unwrap();
    let show = show.show.unwrap();

    assert_eq!(
        ids(&show.ids),
        ExternalIds::from_iter([
            ExternalId::tmdb(70523),
            imdb("tt5753856"),
            ExternalId::tvdb(334_824)
        ])
    );
    assert_eq!(show.title, "Dark");
    assert_eq!(show.first_air_date.as_deref(), Some("2017-12-01"));
    assert_eq!(show.seasons, [1, 2, 3]);
}

#[tokio::test]
async fn a_season_has_its_episodes() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let season = tmdb
        .season(&season_asked(&ExternalId::tmdb(70523), 1))
        .await
        .unwrap();
    let season = season.season.unwrap();

    assert_eq!(season.number, 1);
    assert_eq!(season.title, "Season 1");
    assert_eq!(ids(&season.ids).get(TVDB), Some(ExternalId::tvdb(727_934)));
    let episodes: Vec<(u32, u32, &str, Option<ExternalId>)> = season
        .episodes
        .iter()
        .map(|e| (e.season, e.number, e.title.as_str(), ids(&e.ids).get(TMDB)))
        .collect();
    assert_eq!(
        episodes,
        [
            (1, 1, "Secrets", Some(ExternalId::tmdb(1_388_538))),
            (1, 2, "Lies", Some(ExternalId::tmdb(1_388_539))),
            (1, 3, "Past and Present", Some(ExternalId::tmdb(1_388_540))),
        ]
    );
    assert_eq!(season.episodes[0].runtime_minutes, Some(52));
}

#[tokio::test]
async fn an_episode_has_its_own_ids() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let asked = EpisodeAsked {
        show: title_id(&ExternalId::tmdb(70523)).into(),
        season: 1,
        episode: 1,
        ..Default::default()
    };
    let episode = tmdb.episode(&asked).await.unwrap();
    let episode = episode.episode.unwrap();

    assert_eq!(
        ids(&episode.ids),
        ExternalIds::from_iter([
            ExternalId::tmdb(1_388_538),
            imdb("tt5753730"),
            ExternalId::tvdb(6_288_618)
        ])
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

    let missing = tmdb.movie(&asked(&ExternalId::tmdb(1))).await.unwrap();

    assert!(missing.movie.is_unset(), "{missing:?}");
}

#[tokio::test]
async fn a_movie_is_found_by_its_imdb_id_and_a_show_by_its_tvdb_id() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let movie = tmdb.movie(&asked(&imdb("tt0078748"))).await.unwrap();
    let show = tmdb.show(&asked(&ExternalId::tvdb(334_824))).await.unwrap();
    let (movie, show) = (movie.movie.unwrap(), show.show.unwrap());

    assert_eq!(
        (ids(&movie.ids).get(TMDB), movie.title.as_str()),
        (Some(ExternalId::tmdb(348)), "Alien")
    );
    assert_eq!(
        (ids(&show.ids).get(TMDB), show.title.as_str()),
        (Some(ExternalId::tmdb(70523)), "Dark")
    );
}

#[tokio::test]
async fn an_id_with_no_title_of_the_asked_kind_on_tmdb_is_not_found() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let nothing = tmdb.movie(&asked(&ExternalId::tvdb(1))).await.unwrap();
    // Alien is a Movie, not a Show.
    let a_movie = tmdb.show(&asked(&imdb("tt0078748"))).await.unwrap();
    // Not an IMDb id, so it isn't sent.
    let not_imdb = tmdb.movie(&asked(&imdb("../movie/348"))).await.unwrap();

    assert!(nothing.movie.is_unset(), "{nothing:?}");
    assert!(a_movie.show.is_unset(), "{a_movie:?}");
    assert!(not_imdb.movie.is_unset(), "{not_imdb:?}");
    assert!(
        fake.requests()
            .iter()
            .all(|request| !request.contains("..")),
        "{:?}",
        fake.requests()
    );
}

#[tokio::test]
async fn seasons_are_looked_up_by_tmdb_ids_only() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);
    let dark = ExternalId::tvdb(334_824);

    let result = tmdb.season(&season_asked(&dark, 1)).await;

    assert!(
        matches!(result, Err(MetadataError::UnsupportedId(ref id)) if *id == dark),
        "{result:?}"
    );
    assert_eq!(fake.requests(), Vec::<String>::new());
}

#[tokio::test]
async fn an_id_in_a_namespace_tmdb_does_not_know_is_unsupported() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);
    let id = ExternalId::new("example", "348").unwrap();

    let result = tmdb.movie(&asked(&id)).await;

    assert!(
        matches!(result, Err(MetadataError::UnsupportedId(ref unsupported)) if *unsupported == id),
        "{result:?}"
    );
    assert_eq!(fake.requests(), Vec::<String>::new());
}

fn imdb(id: &str) -> ExternalId {
    ExternalId::new(IMDB, id).unwrap()
}

#[tokio::test]
async fn a_rejected_api_key_is_a_failure_not_a_missing_title() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, "wrong");

    let result = tmdb.movie(&asked(&ExternalId::tmdb(348))).await;

    let Err(MetadataError::Failed(error)) = result else {
        panic!("expected a failure, got {result:?}");
    };
    assert!(error.to_string().contains("API key"), "{error}");
}

#[tokio::test]
async fn an_image_is_fetched_by_the_path_tmdb_gave() {
    let fake = FakeTmdb::start().await;
    let tmdb = provider(&fake, API_KEY);

    let asked = ImageAsked {
        path: "/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg".to_owned(),
        ..Default::default()
    };
    let image = tmdb.image(&asked).await.unwrap().image.unwrap();

    assert_eq!(image.content_type, "image/jpeg");
    assert!(image.bytes.starts_with(b"\xff\xd8"));
    assert_eq!(
        fake.requests(),
        ["/t/p/original/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg"]
    );
}
