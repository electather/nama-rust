//! The TheIntroDB Installed Plugin: a Segment source asking TheIntroDB's v4 API where a Movie's or
//! Episode's Intro, Recap and Credits are. The Server builds it for wasm32 and bundles it in its
//! binary.
//!
//! TheIntroDB's terms (section 4.3, read 2026-11-20) let self-hosted media servers fetch and keep
//! timestamps for their own library, so long as they don't expose them through a public or shared
//! API, redistribute them, or build a database from them.

use nama_pdk::{
    Manifest, OutsideService, Permission, Refusal, Response, SegmentKind, SegmentResult,
    SegmentResults, SegmentSearch, SegmentSource, SettingKind, TitleKind, TryAgainIn,
    extism_pdk::{Error, HttpRequest, json},
    kernel, services,
};
use serde::Deserialize;
use std::time::Duration;

const THEINTRODB: OutsideService = OutsideService::new("TheIntroDB").credential("API key");

const MEDIA_URL: &str = "https://api.theintrodb.org/v4/media";

const API_KEY: &str = "api_key";

/// The Plugin-wide Store entry keeping when the requests of the last [`WINDOW_MS`] were sent.
const SENT_KEY: &str = "sent";

/// TheIntroDB's rate limit window.
const WINDOW_MS: u64 = 10_000;
/// How many `/media` requests TheIntroDB takes in a [`WINDOW_MS`] without an API key.
const ANONYMOUS_LIMIT: usize = 30;
/// How many `/media` requests TheIntroDB takes in a [`WINDOW_MS`] with an API key.
const KEYED_LIMIT: usize = 120;

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        name: "TheIntroDB".to_owned(),
        // TheIntroDB asks for attribution with a link wherever its data is used.
        description: "Finds where Intros, Recaps and Credits are on TheIntroDB (theintrodb.org)."
            .to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::SEGMENT_SOURCE)],
        allowed_hosts: vec!["api.theintrodb.org".to_owned()],
        permissions: vec![Permission::PERMISSION_STORE.into()],
        settings: vec![nama_pdk::setting_field(
            API_KEY,
            "API key",
            "Your TheIntroDB API key, from your account's settings. It raises the rate limit from \
             30 to 120 requests every 10 seconds.",
            SettingKind::SETTING_KIND_SECRET,
        )],
        ..nama_pdk::manifest("theintrodb", env!("CARGO_PKG_VERSION"))
    }
}

/// Serves Intros, Recaps and Credits from TheIntroDB.
struct TheIntroDb;

impl SegmentSource for TheIntroDb {
    fn search(search: SegmentSearch) -> Result<SegmentResults, Error> {
        let Some(url) = media_url(&search) else {
            return Ok(SegmentResults::default());
        };
        let api_key = nama_pdk::setting(API_KEY)?.map(|key| key.trim().to_owned());
        let mut request = HttpRequest::new(url);
        if let Some(api_key) = &api_key {
            request = request.with_header("Authorization", format!("Bearer {api_key}"));
        }
        pace(if api_key.is_some() {
            KEYED_LIMIT
        } else {
            ANONYMOUS_LIMIT
        })?;
        let response = THEINTRODB.send(&request, None::<String>)?;
        Ok(SegmentResults {
            segments: segments(answered(response)?),
            ..SegmentResults::default()
        })
    }
}

nama_pdk::segment_source!(TheIntroDb);

/// The `/media` URL asking about the title `search` describes, by the first of its TVDB, TMDB and
/// IMDb ids, or `None` when it has none of them, or is an Episode without its numbers.
fn media_url(search: &SegmentSearch) -> Option<String> {
    let (namespace, id) = ["tvdb", "tmdb", "imdb"].into_iter().find_map(|namespace| {
        let id = nama_pdk::title_id(&search.ids, namespace)?;
        // Anything else could add query parameters; TheIntroDB's ids are all digits or `tt`.
        id.bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
            .then_some((namespace, id))
    })?;
    // Without numbers TheIntroDB would answer about a Movie.
    let episode = if search.kind.as_known() == Some(TitleKind::TITLE_KIND_SHOW) {
        let (season, episode) = search.season.zip(search.episode)?;
        format!("&season={season}&episode={episode}")
    } else {
        String::new()
    };
    let duration_ms = search.duration_ms;
    Some(format!(
        "{MEDIA_URL}?{namespace}_id={id}{episode}&duration_ms={duration_ms}"
    ))
}

/// Records a request about to be sent, or holds it back when `limit` were sent in the last
/// [`WINDOW_MS`], since each call is a fresh instance that can't wait. Without the Store it can't
/// count, and sends anyway (ADR 0071: the Plugin carries on).
///
/// # Errors
/// [`TryAgainIn`] the whole seconds until a counted request leaves the window, when `limit` were
/// sent in the last [`WINDOW_MS`].
fn pace(limit: usize) -> Result<(), Error> {
    let Ok(kept) = kernel::store_get(SENT_KEY, None) else {
        return Ok(());
    };
    let now = nama_pdk::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let now = u64::try_from(now).unwrap_or(u64::MAX);
    let mut sent: Vec<u64> = kept
        .and_then(|kept| json::from_slice(&kept).ok())
        .unwrap_or_default();
    sent.retain(|&at| now.saturating_sub(at) < WINDOW_MS);
    // Kept in the order they were sent, so this one leaving brings the count under `limit`.
    let leaving = sent
        .len()
        .checked_sub(limit)
        .and_then(|older| sent.get(older));
    if let Some(&leaving) = leaving {
        let leaves_in_ms = leaving.saturating_add(WINDOW_MS) - now;
        return Err(TryAgainIn(Duration::from_secs(leaves_in_ms.div_ceil(1000))).into());
    }
    sent.push(now);
    // Unkept, this request goes uncounted, which TheIntroDB's own 429 still catches.
    let _not_kept: Result<(), Error> = kernel::store_put(SENT_KEY, None, &json::to_vec(&sent)?);
    Ok(())
}

/// What TheIntroDB's `/media` answers, by Segment kind. It also answers `preview`, which isn't
/// one.
#[derive(Deserialize, Default)]
struct Media {
    #[serde(default)]
    intro: Vec<Stretch>,
    #[serde(default)]
    recap: Vec<Stretch>,
    #[serde(default)]
    credits: Vec<Stretch>,
}

#[derive(Deserialize)]
struct Stretch {
    /// `None` when it starts at the start.
    start_ms: Option<u64>,
    /// `None` when it runs to the end.
    end_ms: Option<u64>,
}

/// The `/media` answer in `response`, empty for a title TheIntroDB doesn't know or has nothing
/// for yet: a 404, or a 400 or 422 for an Episode or id it can't place.
///
/// # Errors
/// [`TryAgainIn`] its `Retry-After`, or else [`WINDOW_MS`], when TheIntroDB answered 429 for its
/// rate limit; any other refusal, or an answer that isn't a `/media` one, as it is.
fn answered(response: Response) -> Result<Media, Error> {
    match response.status_code() {
        400 | 404 | 422 => Ok(Media::default()),
        _ => THEINTRODB.accepted(response).map_err(rate_limited)?.json(),
    }
}

/// `refusal` as [`TryAgainIn`] when it is TheIntroDB's 429.
fn rate_limited(refusal: Refusal) -> Error {
    if refusal.status() == 429 {
        let wait = refusal.retry_after_seconds().unwrap_or(WINDOW_MS / 1000);
        TryAgainIn(Duration::from_secs(wait)).into()
    } else {
        refusal.into()
    }
}

/// The earliest stretch of each kind in `media`, leaving out those ending at 0, and Credits
/// starting at 0 or unset, which TheIntroDB answers for "no segment".
fn segments(media: Media) -> Vec<SegmentResult> {
    [
        (SegmentKind::SEGMENT_KIND_INTRO, media.intro),
        (SegmentKind::SEGMENT_KIND_RECAP, media.recap),
        (SegmentKind::SEGMENT_KIND_CREDITS, media.credits),
    ]
    .into_iter()
    .filter_map(|(kind, stretches)| {
        let stretch = (stretches.into_iter())
            .filter(|stretch| {
                stretch.end_ms != Some(0)
                    && (kind != SegmentKind::SEGMENT_KIND_CREDITS
                        || stretch.start_ms.is_some_and(|start| start != 0))
            })
            .min_by_key(|stretch| stretch.start_ms.unwrap_or_default())?;
        Some(SegmentResult {
            kind: kind.into(),
            start_ms: stretch.start_ms.unwrap_or_default(),
            end_ms: stretch.end_ms,
            ..SegmentResult::default()
        })
    })
    .collect()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
