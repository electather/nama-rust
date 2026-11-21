use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, TitleMethod};

use super::{
    show,
    tests::{Answering, Answers, HAS, dark},
};
use crate::{
    library::{
        Decided, TiedTitle, TitleSources, UnmatchedReason,
        scripted::{ScriptedSource, SourcesAsked},
    },
    matching::{
        Miss, PendingEpisode,
        testing::{Fixture, episode, matched, no_recognisers},
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
    shows
        .write(SEASON_3, &[episode("The Office", None, 3, 1)])
        .await;

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
        .write(
            "The Office/Season 01/The Office - S01E05.mkv",
            &[episode("The Office", None, 1, 5)],
        )
        .await;

    media.scan(&media.shows).await;

    assert_eq!(tied(&media).await, [Some(both_offices())]);
}

#[tokio::test]
async fn a_show_whose_episodes_no_tied_show_has_all_of_stays_ambiguous() {
    let media = Fixture::new().await;
    // Both Offices have S01E01; the UK one has no Season 3, the US one's has only Episode 1.
    media
        .shows
        .write(
            "The Office/Season 01/The Office - S01E01.mkv",
            &[episode("The Office", None, 1, 1)],
        )
        .await;
    media
        .shows
        .write(
            "The Office/Season 03/The Office - S03E02.mkv",
            &[episode("The Office", None, 3, 2)],
        )
        .await;

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
    let season_3 = [episode("The Office", None, 3, 1)];
    shows.write(SEASON_3, &season_3).await;
    media.scan(shows).await;
    // Before the tie-break, both Offices left this file Ambiguous.
    let item = media.item(shows, SEASON_3).await;
    let office = |id| TiedTitle {
        id,
        title: "The Office".to_owned(),
        original_title: None,
        year: None,
    };
    let ambiguous = UnmatchedReason::Ambiguous(both_offices().into_iter().map(office).collect());
    let decided = vec![(item, Decided::Unmatched(Some(ambiguous)))];
    media.contents.record(decided).await.unwrap();
    assert_eq!(tied(&media).await, [Some(both_offices())]);

    shows.reparse(SEASON_3, &season_3).await;
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [matched(2_316_301, "Gay Witch Hunt")]
    );
}

/// A source under `aa` whose search ties one Show `Dark` per entry of `answering`, `aa-1` first,
/// each answering as its entry says: its Show, then its Season 1, of one Episode. An entry that
/// fails fails that lookup for every Show, though the tie break stops at the first failure.
fn tying(answering: &[Answering]) -> (TitleSources, SourcesAsked) {
    let asked = SourcesAsked::default();
    let mut source = ScriptedSource::new("aa", &asked);
    for (index, &(show, season)) in answering.iter().enumerate() {
        let id = format!("aa-{}", index + 1);
        source = source.found(&[&id], "Dark");
        source = show.script(source, &[TitleMethod::Show], |s| {
            s.with_show(&[&id], "Dark", &[1])
        });
        source = season.script(source, &[TitleMethod::Season], |s| {
            s.with_season(&id, 1, &[1])
        });
    }
    let plugins = Registry::new();
    plugins.enable(Arc::new(source)).unwrap();
    (TitleSources::new(plugins.extension_points()), asked)
}

/// The id of the Show `Dark` its one Episode S01E01 is in, its search tied.
async fn tied_show(metadata: &TitleSources) -> Result<ExternalId, Miss> {
    let pilot = PendingEpisode {
        item: 1,
        season: 1,
        episode: 1,
        ids: Vec::new(),
    };
    let key = dark(None);
    let found = show(metadata, &no_recognisers(), &key, &[], &[pilot]).await?;
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
        sources_asked.lines(),
        [
            "aa search",
            "aa show aa-1",
            "aa show aa-2",
            "aa season aa-2 1"
        ]
    );
}

#[tokio::test]
async fn a_tied_show_not_listing_the_season_is_asked_for_none_of_its_seasons() {
    let (metadata, sources_asked) = tying(&[HAS, HAS]);
    let key = dark(None);
    let second_season = PendingEpisode {
        item: 1,
        season: 2,
        episode: 1,
        ids: Vec::new(),
    };

    let found = show(&metadata, &no_recognisers(), &key, &[], &[second_season]).await;

    assert!(matches!(
        found,
        Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
    ));
    assert_eq!(
        sources_asked.lines(),
        ["aa search", "aa show aa-1", "aa show aa-2"]
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

    let movie = super::movie(&metadata, &no_recognisers(), &dark(None), &[]).await;

    assert!(matches!(
        movie,
        Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
    ));
    assert_eq!(sources_asked.lines(), ["aa search"]);
}
