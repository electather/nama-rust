use std::sync::{Arc, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Candidate, Episode, SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show,
        ShowAnswer, TitleAsked, TitleId,
    },
    metadata::{ExternalId, MetadataError, TitleSource},
};

use super::{
    show,
    tests::{Answering, Answers, Asked, HAS, asked, dark},
};
use crate::{
    library::{Decided, TitleSources, UnmatchedReason},
    matching::{
        Miss, PendingEpisode,
        recognisers::Recognisers,
        tests::{Fixture, matched},
    },
    plugins::Registry,
};

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

/// A title source under `aa` whose search ties one Show `Dark` per entry of `answering`, `aa-1`
/// first, each looked up as its entry says: its Show, then its Season, of one Episode.
struct Tying {
    answering: Vec<Answering>,
    asked: Asked,
}

impl Tying {
    fn answer<T: Default>(
        &self,
        asked: String,
        answers: Answers,
        found: T,
    ) -> Result<T, MetadataError> {
        let mut log = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        log.push(asked);
        match answers {
            Answers::Has => Ok(found),
            Answers::NotFound => Ok(T::default()),
            Answers::Fails => Err(MetadataError::Failed("unreachable".into())),
        }
    }

    /// How the Show `aa-<value>` answers: its lookup, then its Season's.
    fn answering(&self, value: &str) -> Answering {
        let index: usize = value.parse().unwrap();
        self.answering[index - 1]
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
        let candidates = (1..=self.answering.len())
            .map(|value| Candidate {
                id: aa(&value.to_string()).into(),
                title: "Dark".to_owned(),
                ..Default::default()
            })
            .collect();
        let found = SearchResults {
            candidates,
            ..Default::default()
        };
        let found = self.answer("search".to_owned(), Answers::Has, found);
        Box::pin(async { found })
    }

    fn show<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        let value = &asked.id.value;
        let show = ShowAnswer {
            show: Show {
                ids: vec![aa(value)],
                title: "Dark".to_owned(),
                seasons: vec![1],
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let (lookup, _) = self.answering(value);
        let found = self.answer(format!("show {value}"), lookup, show);
        Box::pin(async { found })
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        let value = &asked.show.value;
        let episode = Episode {
            season: asked.season,
            number: 1,
            ..Default::default()
        };
        let season = SeasonAnswer {
            season: Season {
                number: asked.season,
                episodes: vec![episode],
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let (_, lookup) = self.answering(value);
        let found = self.answer(format!("season {value}"), lookup, season);
        Box::pin(async { found })
    }
}

struct TyingPlugin(Vec<Answering>, Asked);

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
            answering: self.0.clone(),
            asked: Arc::clone(&self.1),
        };
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(source));
        Ok(())
    }
}

/// The [`Tying`] source, answering as `answering` says, and what it was asked.
fn tying(answering: &[Answering]) -> (TitleSources, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    let plugin = TyingPlugin(answering.to_vec(), Arc::clone(&asked));
    plugins.enable(Arc::new(plugin)).unwrap();
    (TitleSources::new(plugins.extension_points()), asked)
}

/// The id of the Show `Dark` its one Episode S01E01 is in, its search tied.
async fn tied_show(metadata: &TitleSources) -> Result<ExternalId, Miss> {
    let pilot = PendingEpisode {
        item: 1,
        season: 1,
        episode: 1,
    };
    let key = dark(None);
    let found = show(metadata, &Recognisers::default(), &key, &[], &[pilot]).await?;
    Ok(found.id)
}

fn aa_id(value: &str) -> ExternalId {
    ExternalId::new("aa", value).unwrap()
}

#[tokio::test]
async fn a_tied_show_the_source_cannot_find_only_drops_out() {
    let (metadata, sources_asked) = tying(&[(Answers::NotFound, Answers::Has), HAS]);

    assert_eq!(tied_show(&metadata).await, Ok(aa_id("2")));
    assert_eq!(
        asked(&sources_asked),
        ["search", "show 1", "show 2", "season 2"]
    );
}

#[tokio::test]
async fn a_tied_shows_season_the_source_cannot_find_only_drops_it_out() {
    let (metadata, _) = tying(&[(Answers::Has, Answers::NotFound), HAS]);

    assert_eq!(tied_show(&metadata).await, Ok(aa_id("2")));
}

#[tokio::test]
async fn a_source_failing_during_the_tie_break_leaves_the_show_undecided() {
    for failing in [
        (Answers::Fails, Answers::Has),
        (Answers::Has, Answers::Fails),
    ] {
        let (metadata, _) = tying(&[failing, HAS]);

        assert_eq!(tied_show(&metadata).await, Err(Miss::Undecided));
    }
}

#[tokio::test]
async fn the_tied_shows_that_have_every_episode_stay_ambiguous_in_their_order() {
    let (metadata, _) = tying(&[HAS, (Answers::Has, Answers::NotFound), HAS]);

    let Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(tied)))) = tied_show(&metadata).await
    else {
        panic!("two Shows have the Episode");
    };
    let ids: Vec<_> = tied.into_iter().map(|title| title.id).collect();
    assert_eq!(ids, [aa_id("1"), aa_id("3")]);
}

#[tokio::test]
async fn an_ambiguous_movie_looks_up_none_of_the_tied_titles() {
    let (metadata, sources_asked) = tying(&[HAS, HAS]);

    let movie = super::movie(&metadata, &Recognisers::default(), &dark(None), &[]).await;

    assert!(matches!(
        movie,
        Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
    ));
    assert_eq!(asked(&sources_asked), ["search"]);
}
