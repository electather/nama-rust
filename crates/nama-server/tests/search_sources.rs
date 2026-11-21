//! Search sources adding batches to `SearchService.Search` (ADR 0083), over a real socket with
//! the TMDB Plugin enabled against a fake TMDB, and fake Search sources of this file's own.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    future,
    sync::{Arc, Mutex, PoisonError},
};

use browsing::Fixture;
use buffa::EnumValue;
use connectrpc::client::{ClientConfig, HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{self, SearchSourceQuery, SearchSourceResults, SearchSourceTitle, TitleId},
    search_source::SearchSource,
};
use nama_proto::nama::v1::{
    Hits, Permission, RequestServiceClient, SearchRequest, SearchResponse, SearchServiceClient,
    TitleKind, TitleStatus, hit, search_response::Part,
};
use nama_server::plugins::Registry;
use server::{
    TestServer,
    requests::Requester,
    search::search,
    tmdb::{ALIEN_IMDB, DUNE, HEAT, THE_OFFICE},
};

/// A query the fake TMDB finds no Movies for, so only a Search source adds any.
const NO_TITLE_MATCHES: &str = "Home Movies";

const AVAILABLE: TitleStatus = TitleStatus::TITLE_STATUS_AVAILABLE;
const REQUESTED: TitleStatus = TitleStatus::TITLE_STATUS_REQUESTED;
const NOT_ON_THIS_SERVER: TitleStatus = TitleStatus::TITLE_STATUS_NOT_ON_THIS_SERVER;

/// What a [`FakeSource`] does when asked.
#[derive(Clone)]
enum Answer {
    Titles(Vec<SearchSourceTitle>),
    Fails,
    Hangs,
}

/// A Search source, enabled as a Plugin of the same id, that answers as scripted and keeps what
/// it was asked.
#[derive(Clone)]
struct FakeSource {
    id: &'static str,
    answer: Answer,
    asked: Arc<Mutex<Vec<SearchSourceQuery>>>,
}

impl FakeSource {
    fn new(id: &'static str, answer: Answer) -> Self {
        Self {
            id,
            answer,
            asked: Arc::default(),
        }
    }

    fn asked(&self) -> Vec<SearchSourceQuery> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SearchSource for FakeSource {
    fn search<'a>(
        &'a self,
        query: &'a SearchSourceQuery,
    ) -> BoxFuture<'a, Result<SearchSourceResults, PluginError>> {
        (self.asked.lock().unwrap_or_else(PoisonError::into_inner)).push(query.clone());
        match self.answer.clone() {
            Answer::Titles(titles) => Box::pin(future::ready(Ok(SearchSourceResults {
                titles,
                ..Default::default()
            }))),
            Answer::Fails => Box::pin(future::ready(Err("the service is down".into()))),
            Answer::Hangs => Box::pin(future::pending()),
        }
    }
}

impl Plugin for FakeSource {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn SearchSource>()],
            ..Manifest::new(PluginId::from_static(self.id))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn SearchSource>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

/// A Movies Library holding Alien, with each of `sources` enabled, in that precedence.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start(sources: &[&FakeSource]) -> Fixture {
    let plugins = Registry::new();
    for source in sources {
        plugins.enable(Arc::new((*source).clone())).unwrap();
    }
    let ids = sources
        .iter()
        .map(|s| PluginId::from_static(s.id))
        .collect();
    plugins.set_provider_precedence::<dyn SearchSource>(ids);
    Fixture::alien_on(TestServer::builder().plugins(plugins)).await
}

/// A title of `kind` a Search source found by `ids`, each a namespace and a value.
fn found(kind: contract::TitleKind, ids: &[(&str, &str)], title: &str) -> SearchSourceTitle {
    SearchSourceTitle {
        kind: kind.into(),
        ids: (ids.iter())
            .map(|(namespace, value)| TitleId {
                namespace: (*namespace).to_owned(),
                value: (*value).to_owned(),
                ..Default::default()
            })
            .collect(),
        title: title.to_owned(),
        ..Default::default()
    }
}

fn movie(ids: &[(&str, &str)], title: &str) -> SearchSourceTitle {
    found(contract::TitleKind::TITLE_KIND_MOVIE, ids, title)
}

/// Each title of `hits` by its title and status.
fn titles_and_statuses(hits: &Hits) -> Vec<(&str, EnumValue<TitleStatus>)> {
    (hits.hits.iter())
        .filter_map(|hit| match &hit.title {
            Some(hit::Title::Movie(movie)) => Some((movie.title.as_str(), hit.status)),
            Some(hit::Title::Show(show)) => Some((show.title.as_str(), hit.status)),
            _ => None,
        })
        .collect()
}

/// Every part a search for Movies streams, as [`search`] gives them, asking for `limit` results.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn search_movies_limited(who: &ClientConfig, query: &str, limit: i32) -> Vec<Part> {
    let request = SearchRequest {
        query: query.to_owned(),
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        limit,
        ..Default::default()
    };
    let client = SearchServiceClient::new(HttpClient::plaintext(), who.clone());
    let mut stream = client.search(request).await.unwrap();
    let mut parts = Vec::new();
    while let Some(response) = stream.message::<SearchResponse>().await.unwrap() {
        parts.extend(response.to_owned_message().part);
    }
    parts
}

#[tokio::test]
async fn a_search_sources_titles_are_available_requested_or_not_on_this_server_by_any_id() {
    let dreams = FakeSource::new(
        "dreams",
        Answer::Titles(vec![
            movie(&[("tmdb", &DUNE.to_string())], "Dune, by its meaning"),
            movie(&[("elsewhere", "1")], "Nobody keeps this one"),
            movie(&[("tmdb", &HEAT.to_string())], "Heat, by its meaning"),
            // Alien's Match knows its IMDb id too.
            movie(&[("imdb", ALIEN_IMDB)], "Alien, by its meaning"),
        ]),
    );
    let fx = start(&[&dreams]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;
    let heat = RequestServiceClient::new(HttpClient::plaintext(), ada.client_config())
        .ask(HEAT)
        .await;

    let parts = search(
        &ada.client_config(),
        NO_TITLE_MATCHES,
        TitleKind::TITLE_KIND_MOVIE,
    )
    .await
    .unwrap();

    let [
        Part::Hits(library),
        Part::Hits(to_request),
        Part::Hits(from_dreams),
        Part::Done(done),
    ] = &parts[..]
    else {
        panic!("the Libraries', the title source's, the Search source's, then Done: {parts:?}");
    };
    assert!(
        library.hits.is_empty() && to_request.hits.is_empty(),
        "{parts:?}"
    );
    assert_eq!(
        titles_and_statuses(from_dreams),
        [
            ("Alien", AVAILABLE.into()),
            ("Heat, by its meaning", REQUESTED.into()),
            ("Dune, by its meaning", NOT_ON_THIS_SERVER.into()),
        ]
    );
    let movies: Vec<_> = (from_dreams.hits.iter())
        .filter_map(|hit| match &hit.title {
            Some(hit::Title::Movie(movie)) => Some(movie),
            _ => None,
        })
        .collect();
    assert_eq!(movies[0].library_id, Some(fx.library));
    assert_eq!(movies[1].request.id, heat.id);
    assert_eq!(movies[2].id.namespace, "tmdb");
    assert_eq!(movies[2].id.value, DUNE.to_string());
    assert!(done.failed_search_sources.is_empty(), "{done:?}");
}

#[tokio::test]
async fn a_not_on_this_server_title_shows_its_poster_through_the_search_sources_plugin() {
    let dune = SearchSourceTitle {
        year: Some(2021),
        poster: Some("/dune.jpg".to_owned()),
        ..movie(&[("tmdb", &DUNE.to_string())], "Dune")
    };
    let dreams = FakeSource::new("dreams", Answer::Titles(vec![dune]));
    let fx = start(&[&dreams]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    let parts = search(
        &ada.client_config(),
        NO_TITLE_MATCHES,
        TitleKind::TITLE_KIND_MOVIE,
    )
    .await
    .unwrap();

    let Some(Part::Hits(from_dreams)) = parts.get(2) else {
        panic!("the Search source's batch third: {parts:?}");
    };
    let Some(hit::Title::Movie(shown)) = &from_dreams.hits[0].title else {
        panic!("Dune: {from_dreams:?}");
    };
    assert_eq!(shown.year, Some(2021));
    assert_eq!(
        shown.poster_url.as_deref(),
        Some("/media/artwork/dreams/%2Fdune.jpg")
    );
}

#[tokio::test]
async fn a_search_source_title_of_a_kind_the_caller_may_not_request_is_dropped() {
    let office = found(
        contract::TitleKind::TITLE_KIND_SHOW,
        &[("tmdb", &THE_OFFICE.to_string())],
        "The Office",
    );
    let dune = movie(&[("tmdb", &DUNE.to_string())], "Dune");
    let dreams = FakeSource::new("dreams", Answer::Titles(vec![office, dune]));
    let fx = start(&[&dreams]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    // Of all kinds, so the source's Show is asked for; Ada may request only Movies, so the title
    // source isn't asked for Shows.
    let parts = search(
        &ada.client_config(),
        NO_TITLE_MATCHES,
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await
    .unwrap();

    let [
        Part::Hits(_),
        Part::Hits(_),
        Part::Hits(from_dreams),
        Part::Done(_),
    ] = &parts[..]
    else {
        panic!("the Libraries', the title source's, the Search source's, then Done: {parts:?}");
    };
    assert_eq!(
        titles_and_statuses(from_dreams),
        [("Dune", NOT_ON_THIS_SERVER.into())]
    );
}

#[tokio::test]
async fn search_sources_batches_follow_the_admins_precedence_leaving_out_titles_sent_before() {
    let first = FakeSource::new(
        "first",
        Answer::Titles(vec![
            // Alien, which the Libraries' batch sent, by an id that batch didn't show.
            movie(&[("imdb", ALIEN_IMDB)], "Alien again"),
            movie(&[("tmdb", &DUNE.to_string())], "Dune"),
        ]),
    );
    let second = FakeSource::new(
        "second",
        Answer::Titles(vec![
            movie(&[("other", "7"), ("tmdb", &DUNE.to_string())], "Dune again"),
            movie(&[("tmdb", &HEAT.to_string())], "Heat"),
        ]),
    );
    let fx = start(&[&second, &first]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    let parts = search(&ada.client_config(), "Alien", TitleKind::TITLE_KIND_MOVIE)
        .await
        .unwrap();

    let [
        Part::Hits(library),
        Part::Hits(_),
        Part::Hits(from_second),
        Part::Hits(from_first),
        Part::Done(_),
    ] = &parts[..]
    else {
        panic!("the Libraries', the title source's, each Search source's, then Done: {parts:?}");
    };
    assert_eq!(titles_and_statuses(library), [("Alien", AVAILABLE.into())]);
    assert_eq!(
        titles_and_statuses(from_second),
        [
            ("Dune again", NOT_ON_THIS_SERVER.into()),
            ("Heat", NOT_ON_THIS_SERVER.into())
        ]
    );
    assert!(from_first.hits.is_empty(), "{from_first:?}");
}

#[tokio::test]
async fn a_search_sources_batch_is_cut_to_the_limit_after_titles_sent_before_are_left_out() {
    let dreams = FakeSource::new(
        "dreams",
        Answer::Titles(vec![
            movie(&[("imdb", ALIEN_IMDB)], "Alien again"),
            movie(&[("tmdb", &DUNE.to_string())], "Dune"),
            movie(&[("tmdb", &HEAT.to_string())], "Heat"),
        ]),
    );
    let fx = start(&[&dreams]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    let parts = search_movies_limited(&ada.client_config(), "Alien", 1).await;

    let [
        Part::Hits(library),
        Part::Hits(_),
        Part::Hits(from_dreams),
        Part::Done(_),
    ] = &parts[..]
    else {
        panic!("the Libraries', the title source's, the Search source's, then Done: {parts:?}");
    };
    assert_eq!(titles_and_statuses(library), [("Alien", AVAILABLE.into())]);
    assert_eq!(
        titles_and_statuses(from_dreams),
        [("Dune", NOT_ON_THIS_SERVER.into())]
    );
    let asked: Vec<_> = dreams.asked().into_iter().map(|q| q.limit).collect();
    assert_eq!(asked, [1]);
}

#[tokio::test]
async fn a_failing_and_a_slow_search_source_are_named_in_done_and_the_others_still_answer() {
    let broken = FakeSource::new("broken", Answer::Fails);
    let slow = FakeSource::new("slow", Answer::Hangs);
    let dreams = FakeSource::new(
        "dreams",
        Answer::Titles(vec![movie(&[("tmdb", &DUNE.to_string())], "Dune")]),
    );
    let fx = start(&[&slow, &dreams, &broken]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    let parts = search(
        &ada.client_config(),
        NO_TITLE_MATCHES,
        TitleKind::TITLE_KIND_MOVIE,
    )
    .await
    .unwrap();

    let [
        Part::Hits(_),
        Part::Hits(_),
        Part::Hits(from_dreams),
        Part::Done(done),
    ] = &parts[..]
    else {
        panic!("the Libraries', the title source's, one Search source's, then Done: {parts:?}");
    };
    assert_eq!(
        titles_and_statuses(from_dreams),
        [("Dune", NOT_ON_THIS_SERVER.into())]
    );
    assert_eq!(done.failed_search_sources, ["slow", "broken"]);
    assert!(!done.title_source_unavailable);
}

#[tokio::test]
async fn a_search_source_is_asked_only_the_query_the_kind_and_the_limit() {
    let dreams = FakeSource::new("dreams", Answer::Titles(Vec::new()));
    let fx = start(&[&dreams]).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    search(
        &ada.client_config(),
        "dreams inside dreams",
        TitleKind::TITLE_KIND_MOVIE,
    )
    .await
    .unwrap();

    let expected = SearchSourceQuery {
        query: "dreams inside dreams".to_owned(),
        kind: contract::TitleKind::TITLE_KIND_MOVIE.into(),
        limit: 20,
        ..Default::default()
    };
    assert_eq!(dreams.asked(), [expected]);
}
