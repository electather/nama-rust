use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    contract::Request,
    fulfiller::{Fulfiller, Movies, Shows},
};

use super::{
    AskedAbout, FulfillerOutcome, RETRIES, ask_fulfiller, enabled_fulfiller, tell_fulfiller,
};
use crate::plugins::{Registry, ScriptedFulfiller};

const ID: PluginId = PluginId::from_static("scripted");
const WAIT: Duration = Duration::from_secs(5);

/// A Show known by its tmdb id alone.
fn tmdb_show() -> AskedAbout<'static> {
    AskedAbout::new("Show", ["tmdb"])
}

/// A Registry whose one Plugin, [`ID`], provides `fulfiller` as the Fulfiller of each kind.
fn enabled(fulfiller: ScriptedFulfiller) -> (Registry, Arc<ScriptedFulfiller>) {
    fulfiller.enabled("scripted")
}

/// What the Fulfiller of Shows in `plugins` does when handed a Request about `title`.
async fn fulfill(
    plugins: &Registry,
    title: &AskedAbout<'_>,
) -> Option<FulfillerOutcome<nama_plugin_api::fulfiller::Fulfilled>> {
    let points = plugins.extension_points();
    let fulfill = <dyn Fulfiller<Shows>>::fulfill;
    ask_fulfiller(&points, title, fulfill, &Request::default()).await
}

/// The reason the Fulfiller of Shows in `plugins` gives for not being asked about a Show known by its tmdb id alone.
async fn lacks_ids_reason(plugins: &Registry) -> String {
    match fulfill(plugins, &tmdb_show()).await {
        Some(FulfillerOutcome::LacksIds { reason, .. }) => reason,
        other => panic!("it was asked: {other:?}"),
    }
}

#[tokio::test]
async fn the_kinds_fulfiller_is_the_enabled_plugin_providing_it() {
    let (plugins, _fulfiller) = enabled(ScriptedFulfiller::default());

    let points = plugins.extension_points();

    assert_eq!(enabled_fulfiller::<Movies>(&points), Some(ID));
    assert_eq!(
        enabled_fulfiller::<Movies>(&Registry::new().extension_points()),
        None
    );
}

#[tokio::test]
async fn a_title_with_no_id_its_fulfiller_needs_isnt_asked_and_says_which_it_needs() {
    let (plugins, fulfiller) = enabled(ScriptedFulfiller::default().needing(&["tvdb"]));

    let outcome = fulfill(&plugins, &tmdb_show()).await;

    let Some(FulfillerOutcome::LacksIds {
        plugin,
        needs,
        reason,
    }) = outcome
    else {
        panic!("it was asked: {outcome:?}");
    };
    assert_eq!((plugin, needs), (ID, vec!["tvdb".to_owned()]));
    assert_eq!(
        reason,
        "scripted needs the Show's tvdb id, and none is known for it"
    );
    assert_eq!(fulfiller.asked(), 0);
}

#[tokio::test]
async fn a_title_lacking_every_id_its_fulfiller_needs_names_them_all() {
    let needing = ScriptedFulfiller::default().needing(&["tvdb", "imdb", "anidb"]);
    let (plugins, _fulfiller) = enabled(needing);

    assert_eq!(
        lacks_ids_reason(&plugins).await,
        "scripted needs one of the Show's tvdb, imdb or anidb ids, and none is known for it"
    );
}

#[tokio::test]
async fn a_title_with_one_id_its_fulfiller_needs_is_asked() {
    let (plugins, fulfiller) = enabled(ScriptedFulfiller::default().needing(&["anidb", "tmdb"]));

    let outcome = fulfill(&plugins, &tmdb_show()).await;

    assert!(
        matches!(&outcome, Some(FulfillerOutcome::Took { plugin, .. }) if *plugin == ID),
        "{outcome:?}"
    );
    assert_eq!(fulfiller.handed(), [0]);
}

#[tokio::test]
async fn a_fulfiller_that_needs_no_id_is_asked_about_a_title_with_none() {
    let (plugins, fulfiller) = enabled(ScriptedFulfiller::default());
    let no_ids = AskedAbout::new("title", []);

    let outcome = fulfill(&plugins, &no_ids).await;

    assert!(
        matches!(outcome, Some(FulfillerOutcome::Took { .. })),
        "{outcome:?}"
    );
    assert_eq!(fulfiller.asked(), 1);
}

#[tokio::test]
async fn asking_a_fulfiller_that_says_to_try_again_returns_its_wait() {
    let (plugins, _fulfiller) = enabled(ScriptedFulfiller::default().trying_again(1, WAIT));

    let outcome = fulfill(&plugins, &tmdb_show()).await;

    assert!(
        matches!(&outcome, Some(FulfillerOutcome::TryAgain { plugin, wait, .. }) if *plugin == ID && *wait == WAIT),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn asking_with_no_fulfiller_enabled_returns_nothing() {
    assert_eq!(fulfill(&Registry::new(), &tmdb_show()).await, None);
}

/// Each Plugin and reason [`tell_fulfiller`] hands its callback.
type NotTold = Arc<Mutex<Vec<(PluginId, String)>>>;

/// Tells the Fulfiller of Shows in `plugins` to stop following a Show known by its tmdb id alone. Returns what
/// telling returned and what it hands its callback, then and later.
async fn unfollow(plugins: &Registry) -> (Option<PluginId>, NotTold) {
    let not_told = NotTold::default();
    let log = Arc::clone(&not_told);
    let points = plugins.extension_points();
    let unfollow = <dyn Fulfiller<Shows>>::unfollow;
    let told = tell_fulfiller(&points, &tmdb_show(), unfollow, &Request::default(), {
        move |plugin: &PluginId, reason: &str| {
            log.lock()
                .unwrap()
                .push((plugin.clone(), reason.to_owned()));
        }
    })
    .await;
    (told, not_told)
}

/// How many times a Fulfiller that says to try again `times` is told to unfollow by
/// [`tell_fulfiller`], once every wait is over, and each reason it handed the callback.
async fn told_again(times: usize) -> (usize, Vec<String>) {
    let (plugins, fulfiller) = enabled(ScriptedFulfiller::default().trying_again(times, WAIT));

    let (now, not_told) = unfollow(&plugins).await;
    // The paused clock jumps through every wait while all else is idle.
    tokio::time::sleep(WAIT * (RETRIES + 1)).await;

    assert_eq!(now, None, "a retry waiting named the Plugin as not told");
    let reasons = not_told.lock().unwrap();
    let reasons = reasons.iter().map(|(_, reason)| reason.clone()).collect();
    (fulfiller.asked(), reasons)
}

#[tokio::test(start_paused = true)]
async fn a_fulfiller_is_asked_again_until_it_acts_without_giving_up() {
    let (asked, gave_up) = told_again(2).await;

    assert_eq!(asked, 3);
    assert!(gave_up.is_empty(), "{gave_up:?}");
}

#[tokio::test(start_paused = true)]
async fn a_fulfiller_that_keeps_saying_to_try_again_is_asked_retries_times_then_given_up_on_once() {
    let (asked, gave_up) = told_again(usize::MAX).await;

    assert_eq!(asked, 1 + RETRIES as usize);
    let [reason] = &gave_up[..] else {
        panic!("gave up {} times", gave_up.len());
    };
    assert!(
        reason.ends_with(&format!("asked again {RETRIES} times")),
        "{reason}"
    );
}

#[tokio::test]
async fn telling_a_fulfiller_that_lacks_ids_fails_or_panics_names_it_and_logs_why() {
    let cases = [
        (
            ScriptedFulfiller::default().needing(&["tvdb"]),
            "scripted needs the Show's tvdb id, and none is known for it",
        ),
        (
            ScriptedFulfiller::default().failing("the service is down"),
            "the service is down",
        ),
        (
            ScriptedFulfiller::default().panicking(),
            "it panicked or was disabled",
        ),
    ];
    for (fulfiller, why) in cases {
        let (plugins, _fulfiller) = enabled(fulfiller);

        let (told, not_told) = unfollow(&plugins).await;

        assert_eq!(told, Some(ID), "{why}");
        assert_eq!(*not_told.lock().unwrap(), [(ID, why.to_owned())]);
    }
}

#[tokio::test]
async fn telling_a_fulfiller_that_acts_names_nothing() {
    let (plugins, fulfiller) = enabled(ScriptedFulfiller::default());

    let (told, not_told) = unfollow(&plugins).await;

    assert_eq!(told, None);
    assert!(not_told.lock().unwrap().is_empty());
    assert_eq!(fulfiller.asked(), 1);
}

#[tokio::test]
async fn telling_with_no_fulfiller_enabled_does_nothing() {
    let (told, not_told) = unfollow(&Registry::new()).await;

    assert_eq!(told, None);
    assert!(not_told.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn disabling_the_fulfiller_ends_a_waiting_retry() {
    let (plugins, fulfiller) = enabled(ScriptedFulfiller::default().trying_again(1, WAIT));
    let (_now, not_told) = unfollow(&plugins).await;

    assert!(plugins.disable(&ID).await);
    tokio::time::sleep(WAIT * 2).await;

    assert_eq!(fulfiller.asked(), 1);
    assert!(not_told.lock().unwrap().is_empty());
}
