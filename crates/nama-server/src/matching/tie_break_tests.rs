use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Candidate, Episode, SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show,
        ShowAnswer, TitleAsked, TitleId,
    },
    metadata::{ExternalId, MetadataError, TitleSource},
};

use super::super::{
    Miss, PendingEpisode, TitleKey,
    recognisers::Recognisers,
    tests::{Fixture, matched},
};
use crate::library::{Decided, Item, TitleSources, UnmatchedReason};

const SEASON_3: &str = "The Office/Season 03/The Office - S03E01.mkv";

/// The TMDB ids of the titles an Unmatched Item's reason says tied, if it says any did.
fn tied_ids(reason: Option<&UnmatchedReason>) -> Option<Vec<ExternalId>> {
    let Some(UnmatchedReason::Ambiguous(tied)) = reason else {
        return None;
    };
    Some(tied.iter().map(|t| t.id.clone()).collect())
}

/// Why each Item of the Shows Library is Unmatched, as [`tied_ids`].
async fn tied(media: &Fixture) -> Vec<Option<Vec<ExternalId>>> {
    let reasons = media.reasons(&media.shows).await;
    reasons.iter().map(|(_, r)| tied_ids(r.as_ref())).collect()
}

fn both_offices() -> Vec<ExternalId> {
    vec![ExternalId::tmdb(2316), ExternalId::tmdb(2996)]
}

#[tokio::test]
async fn an_episode_only_one_tied_show_has_matches_that_show() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    // No year, so the US and the UK Office tie; only the US one has a Season 3.
    shows.write(SEASON_3);

    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [matched(2_316_301, "Gay Witch Hunt")]
    );
}

#[tokio::test]
async fn an_episode_no_tied_show_has_stays_ambiguous_with_every_tied_show() {
    let media = Fixture::new().await;
    // Both Offices' Season 1 has 2 Episodes.
    media
        .shows
        .write("The Office/Season 01/The Office - S01E05.mkv");

    media.scan(&media.shows).await;

    assert_eq!(tied(&media).await, [Some(both_offices())]);
}

#[tokio::test]
async fn a_show_whose_episodes_no_tied_show_has_all_of_stays_ambiguous() {
    let media = Fixture::new().await;
    // The US Office's Season 3 has only Episode 1.
    media.shows.write(SEASON_3);
    media
        .shows
        .write("The Office/Season 03/The Office - S03E02.mkv");

    media.scan(&media.shows).await;

    assert_eq!(
        tied(&media).await,
        [Some(both_offices()), Some(both_offices())]
    );
}

#[tokio::test]
async fn a_newer_name_parser_matches_an_episode_only_one_tied_show_has() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SEASON_3);
    media.scan(shows).await;
    // Before the tie-break, both Offices left this file Ambiguous.
    let item = media.item(shows, SEASON_3).await;
    let ambiguous = UnmatchedReason::Ambiguous(Vec::new());
    let decided = vec![(item, Decided::Unmatched(Some(ambiguous)))];
    media.contents.record(decided).await.unwrap();

    media.contents.read_by_older_parser(shows.id).await;
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [matched(2_316_301, "Gay Witch Hunt")]
    );
}

/// How the [`Tying`] source answers a lookup of one of its two Shows.
#[derive(Clone, Copy)]
enum Lookup {
    Has,
    NotFound,
    Fails,
}

/// A title source under `aa` whose search ties the titles `aa-1` and `aa-2`, each a Show `Dark`
/// with Season 1 of one Episode, looked up as `lookups` says.
struct Tying {
    lookups: [Lookup; 2],
    asked: Arc<Mutex<Vec<String>>>,
}

impl Tying {
    fn log(&self, asked: String) {
        let mut log = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        log.push(asked);
    }
}

fn aa(value: &str) -> TitleId {
    TitleId {
        namespace: "aa".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

impl TitleSource for Tying {
    fn namespace(&self) -> &'static str {
        "aa"
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        self.log("search".to_owned());
        let candidate = |value| Candidate {
            id: aa(value).into(),
            title: "Dark".to_owned(),
            ..Default::default()
        };
        let candidates = vec![candidate("1"), candidate("2")];
        Box::pin(async {
            Ok(SearchResults {
                candidates,
                ..Default::default()
            })
        })
    }

    fn show<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        self.log(format!("show {}", asked.id.value));
        let lookup = if asked.id.value == "1" {
            self.lookups[0]
        } else {
            self.lookups[1]
        };
        let show = Show {
            ids: vec![aa(&asked.id.value)],
            title: "Dark".to_owned(),
            seasons: vec![1],
            ..Default::default()
        };
        let answer = match lookup {
            Lookup::Has => Ok(ShowAnswer {
                show: show.into(),
                ..Default::default()
            }),
            Lookup::NotFound => Ok(ShowAnswer::default()),
            Lookup::Fails => Err(MetadataError::Failed("unreachable".into())),
        };
        Box::pin(async { answer })
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        let episode = Episode {
            season: asked.season,
            number: 1,
            ..Default::default()
        };
        let season = Season {
            number: asked.season,
            episodes: vec![episode],
            ..Default::default()
        };
        Box::pin(async {
            Ok(SeasonAnswer {
                season: season.into(),
                ..Default::default()
            })
        })
    }
}

struct TyingPlugin([Lookup; 2], Arc<Mutex<Vec<String>>>);

impl Plugin for TyingPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static("aa"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = Tying {
            lookups: self.0,
            asked: Arc::clone(&self.1),
        };
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(source));
        Ok(())
    }
}

/// The [`Tying`] source, looking its Shows up as `lookups` says, and what it was asked.
fn tying(lookups: [Lookup; 2]) -> (TitleSources, Arc<Mutex<Vec<String>>>) {
    let plugins = crate::plugins::Registry::new();
    let asked = Arc::default();
    let plugin = TyingPlugin(lookups, Arc::clone(&asked));
    plugins.enable(Arc::new(plugin)).unwrap();
    (TitleSources::new(plugins.extension_points()), asked)
}

/// `Dark`, with no year: a Movie, or the Show of an Episode.
fn dark() -> TitleKey {
    let item = Item::Movie {
        title: "Dark".to_owned(),
        year: None,
        hint: None,
    };
    TitleKey {
        name: item.name(),
        pin: None,
    }
}

/// The Show [`dark`] names, its one Episode S01E01.
async fn show(metadata: &TitleSources) -> Result<ExternalId, Miss> {
    let pilot = PendingEpisode {
        item: 1,
        season: 1,
        episode: 1,
    };
    let found = super::show(metadata, &Recognisers::default(), &dark(), &[], &[pilot]).await?;
    Ok(found.id)
}

fn asked(asked: &Mutex<Vec<String>>) -> Vec<String> {
    asked.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[tokio::test]
async fn a_tied_show_the_source_cannot_find_only_drops_out() {
    let (metadata, sources_asked) = tying([Lookup::NotFound, Lookup::Has]);

    assert_eq!(
        show(&metadata).await,
        Ok(ExternalId::new("aa", "2").unwrap())
    );
    assert_eq!(asked(&sources_asked), ["search", "show 1", "show 2"]);
}

#[tokio::test]
async fn a_source_failing_during_the_tie_break_leaves_the_show_undecided() {
    let (metadata, _) = tying([Lookup::Fails, Lookup::Has]);

    assert_eq!(show(&metadata).await, Err(Miss::Undecided));
}

#[tokio::test]
async fn an_ambiguous_movie_looks_up_none_of_the_tied_titles() {
    let (metadata, sources_asked) = tying([Lookup::Has, Lookup::Has]);

    let movie = super::movie(&metadata, &Recognisers::default(), &dark(), &[]).await;

    assert!(matches!(
        movie,
        Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
    ));
    assert_eq!(asked(&sources_asked), ["search"]);
}
