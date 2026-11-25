//! The Trakt Installed Plugin: a Watch sync that adds each User's plays to the history of the
//! Trakt account they linked in Personal settings, removes the titles they mark unplayed, and
//! brings the plays made elsewhere back. The Server builds it for wasm32 and bundles it in its
//! binary.

use std::{
    collections::{BTreeMap, HashMap},
    hash::BuildHasher,
};

use jiff::Timestamp;
use nama_pdk::{
    ImportAsked, Imported, LinkPolled, LinkStarted, Manifest, OutsideService, Permission,
    PollLinkAsked, Response, ScrobbleAsked, Scrobbled, SettingField, SettingKind, StartLinkAsked,
    SyncedWatchState, TitleId, WatchSync,
    extism_pdk::{
        Error, HttpRequest,
        json::{self, json},
    },
    kernel,
    nama_plugin_proto::nama::plugin::v1::TitleRefKind,
    required_setting, services, setting_field, title_id,
};
use serde::{Deserialize, Serialize};

/// Where Trakt's API, its OAuth endpoints included, is.
const API_URL: &str = "https://api.trakt.tv";

const CLIENT_ID: &str = "client_id";
const CLIENT_SECRET: &str = "client_secret";
const ACCESS_TOKEN: &str = "access_token";
const REFRESH_TOKEN: &str = "refresh_token";

/// Trakt's API, whose 401 means the User's access token.
const TRAKT: OutsideService = OutsideService::new("Trakt").credential("access token");

/// The per-User Store entry keeping the [`Refreshed`] tokens.
const TOKENS_KEY: &str = "tokens";

/// The per-User Store entry keeping the [`Plays`] added lately.
const PLAYS_KEY: &str = "plays";

/// How long after adding a play of a title another played change of it counts as the same play.
/// The Server tells a played title again with each position report, every 10 s while it plays, so
/// a rewatch within this long isn't a play of its own.
const PLAY_WINDOW_MS: i64 = 6 * 60 * 60 * 1000;

/// How many plays each page of `/sync/history` asks for.
const HISTORY_PAGE: u32 = 100;

/// Trakt's redirect URI for an app with no web page, which refreshing a token names.
const NO_REDIRECT: &str = "urn:ietf:wg:oauth:2.0:oob";

const SETUP: &str = "\
1. Sign in to [Trakt](https://trakt.tv/) and [create an API app](https://trakt.tv/oauth/applications/new), with `urn:ietf:wg:oauth:2.0:oob` as its redirect URI.
2. Copy its Client ID and Client Secret into the settings below.
3. Grant the Store, which keeps each User's link working as Trakt renews it.

Each User then links their own Trakt account in Personal settings.
";

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

    nama_pdk::watch_sync!(super::Trakt);
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let app_setting = |key: &str, label: &str, description: &str| SettingField {
        required: true,
        ..setting_field(key, label, description, SettingKind::SETTING_KIND_SECRET)
    };
    // Filled in by linking, not typed.
    let token = |key: &str, label: &str| {
        let description = "Set by linking your Trakt account.";
        setting_field(key, label, description, SettingKind::SETTING_KIND_SECRET)
    };
    Manifest {
        name: "Trakt".to_owned(),
        description: "Syncs what each User watches with their own Trakt account.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        setup_markdown: SETUP.to_owned(),
        points: vec![nama_pdk::point(services::WATCH_SYNC)],
        allowed_hosts: vec!["api.trakt.tv".to_owned()],
        permissions: vec![Permission::PERMISSION_STORE.into()],
        settings: vec![
            app_setting(CLIENT_ID, "Client ID", "Your Trakt API app's Client ID."),
            app_setting(
                CLIENT_SECRET,
                "Client Secret",
                "Your Trakt API app's Client Secret.",
            ),
        ],
        personal_settings: vec![
            token(ACCESS_TOKEN, "Access token"),
            token(REFRESH_TOKEN, "Refresh token"),
        ],
        ..nama_pdk::manifest("trakt", env!("CARGO_PKG_VERSION"))
    }
}

/// Syncs each User's Watch State with their Trakt account's history.
struct Trakt;

impl WatchSync for Trakt {
    /// Adds each newly played title to the User's Trakt history as of when it was played, and
    /// removes each one marked unplayed. Position reports of an unplayed title are skipped.
    fn scrobble(asked: ScrobbleAsked) -> Result<Scrobbled, Error> {
        let mut account = Account::of(asked.user_id, &asked.personal_settings)?;
        let mut plays = Plays::kept(asked.user_id);
        let (mut watched, mut unwatched) = (History::default(), History::default());
        for change in &asked.changes {
            let forgotten = !change.played && change.position_ms == 0 && change.duration_ms == 0;
            if change.played && plays.is_new(change) {
                watched.add(change, rfc3339(change.changed_at_ms));
            } else if forgotten {
                plays.forget(change);
                unwatched.add(change, None);
            }
        }
        account.post_history("/sync/history", &watched)?;
        account.post_history("/sync/history/remove", &unwatched)?;
        plays.keep(asked.user_id);
        Ok(Scrobbled::default())
    }

    /// The plays in the User's Trakt history since the last import, everything on the first; the
    /// cursor is when Trakt last saw any of their activity, so an unchanged account costs one ask.
    /// Trakt lists no removals, so marking unplayed there doesn't come back.
    fn import(asked: ImportAsked) -> Result<Imported, Error> {
        let mut account = Account::of(asked.user_id, &asked.personal_settings)?;
        let activities: LastActivities =
            account.send("GET", "/sync/last_activities", None)?.json()?;
        let changes = if activities.all == asked.since {
            Vec::new()
        } else {
            account.history_since(asked.since.parse().ok())?
        };
        Ok(Imported {
            changes,
            cursor: activities.all,
            ..Imported::default()
        })
    }

    fn start_link(_asked: StartLinkAsked) -> Result<LinkStarted, Error> {
        let body = json!({ "client_id": required_setting(CLIENT_ID)? }).to_string();
        let answer = TRAKT.send(&api_request("POST", "/oauth/device/code")?, Some(body))?;
        let code: LinkCode = TRAKT.accepted(answer)?.json()?;
        Ok(LinkStarted {
            user_code: code.user_code,
            verification_url: code.verification_url,
            expires_in_seconds: code.expires_in,
            interval_seconds: code.interval,
            device_code: code.device_code,
            ..LinkStarted::default()
        })
    }

    fn poll_link(asked: PollLinkAsked) -> Result<LinkPolled, Error> {
        let body = json!({
            "code": asked.device_code,
            "client_id": required_setting(CLIENT_ID)?,
            "client_secret": required_setting(CLIENT_SECRET)?,
        })
        .to_string();
        let answer = TRAKT.send(&api_request("POST", "/oauth/device/token")?, Some(body))?;
        match answer.status_code() {
            // Not entered yet, or polled too fast.
            400 | 429 => Ok(LinkPolled::default()),
            // Unknown, already used, expired, or refused by the User.
            404 | 409 | 410 | 418 => Ok(LinkPolled {
                expired: true,
                ..LinkPolled::default()
            }),
            _ => {
                let tokens: Tokens = TRAKT.accepted(answer)?.json()?;
                Ok(LinkPolled {
                    personal_settings: [
                        (ACCESS_TOKEN.to_owned(), tokens.access_token),
                        (REFRESH_TOKEN.to_owned(), tokens.refresh_token),
                    ]
                    .into_iter()
                    .collect(),
                    ..LinkPolled::default()
                })
            }
        }
    }
}

/// A request to Trakt's API at `path`, naming the Admin's app.
fn api_request(method: &str, path: &str) -> Result<HttpRequest, Error> {
    Ok(HttpRequest::new(format!("{API_URL}{path}"))
        .with_method(method)
        .with_header("Content-Type", "application/json")
        .with_header("trakt-api-version", "2")
        .with_header("trakt-api-key", required_setting(CLIENT_ID)?))
}

/// `ms` since the Unix epoch as Trakt reads a time, or `None`, meaning now, if out of range.
fn rfc3339(ms: i64) -> Option<String> {
    Timestamp::from_millisecond(ms)
        .ok()
        .map(|at| at.to_string())
}

/// A User's linked Trakt account, with the tokens its requests go with.
struct Account {
    user_id: i64,
    /// The refresh token the User's Personal settings hold, which tells a link apart from the one
    /// before it.
    linked: String,
    tokens: Tokens,
}

impl Account {
    /// The account the User `user_id` linked, with the tokens the Store keeps for that link if it
    /// keeps any, else those their Personal settings hold.
    fn of<S: BuildHasher>(
        user_id: i64,
        personal_settings: &HashMap<String, String, S>,
    ) -> Result<Self, Error> {
        let setting = |key| personal_settings.get(key).filter(|value| !value.is_empty());
        let (Some(access_token), Some(refresh_token)) =
            (setting(ACCESS_TOKEN), setting(REFRESH_TOKEN))
        else {
            return Err(Error::msg(
                "the User hasn't linked a Trakt account in Personal settings",
            ));
        };
        let linked = refresh_token.clone();
        let tokens = Refreshed::kept(user_id)
            .filter(|kept| kept.linked == linked)
            .map_or_else(
                || Tokens {
                    access_token: access_token.clone(),
                    refresh_token: refresh_token.clone(),
                },
                |kept| kept.tokens,
            );
        Ok(Self {
            user_id,
            linked,
            tokens,
        })
    }

    /// Trakt's 2xx answer to `method` `path` with `body`. A 401 renews the access token and asks
    /// once more.
    fn send(&mut self, method: &str, path: &str, body: Option<&str>) -> Result<Response, Error> {
        let mut answer = self.send_once(method, path, body)?;
        if answer.status_code() == 401 {
            self.refresh()?;
            answer = self.send_once(method, path, body)?;
        }
        Ok(TRAKT.accepted(answer)?)
    }

    fn send_once(&self, method: &str, path: &str, body: Option<&str>) -> Result<Response, Error> {
        let bearer = format!("Bearer {}", self.tokens.access_token);
        let request = api_request(method, path)?.with_header("Authorization", bearer);
        TRAKT.send(&request, body)
    }

    /// Trades the refresh token for new tokens, which the Store then keeps for this link. Without
    /// the Store, the next renewal offers the spent refresh token and Trakt refuses it.
    fn refresh(&mut self) -> Result<(), Error> {
        let body = json!({
            "refresh_token": self.tokens.refresh_token,
            "client_id": required_setting(CLIENT_ID)?,
            "client_secret": required_setting(CLIENT_SECRET)?,
            "redirect_uri": NO_REDIRECT,
            "grant_type": "refresh_token",
        })
        .to_string();
        let answer = TRAKT.send(&api_request("POST", "/oauth/token")?, Some(body))?;
        if matches!(answer.status_code(), 400 | 401) {
            return Err(Error::msg(
                "Trakt no longer takes this User's link; they link Trakt again in Personal \
                 settings",
            ));
        }
        self.tokens = TRAKT.accepted(answer)?.json()?;
        Refreshed {
            linked: self.linked.clone(),
            tokens: self.tokens.clone(),
        }
        .keep(self.user_id);
        Ok(())
    }

    /// POSTs `history` to `path`, unless it is empty.
    fn post_history(&mut self, path: &str, history: &History) -> Result<(), Error> {
        if history.movies.is_empty() && history.episodes.is_empty() {
            return Ok(());
        }
        self.send("POST", path, Some(&json::to_string(history)?))
            .map(drop)
    }

    /// Every play in the User's history watched at or after `since`, or ever without it, page by
    /// page, skipping those with neither a TMDB nor a TVDB id.
    fn history_since(&mut self, since: Option<Timestamp>) -> Result<Vec<SyncedWatchState>, Error> {
        let start_at = since.map_or_else(String::new, |since| format!("&start_at={since}"));
        let mut changes = Vec::new();
        for page in 1.. {
            let path = format!("/sync/history?page={page}&limit={HISTORY_PAGE}{start_at}");
            let answer = self.send("GET", &path, None)?;
            let plays: Vec<Play> = answer.json()?;
            changes.extend(plays.iter().filter_map(Play::synced));
            let pages: u32 = (answer.header("X-Pagination-Page-Count"))
                .and_then(|pages| pages.parse().ok())
                .unwrap_or(page);
            if plays.is_empty() || page >= pages {
                break;
            }
        }
        Ok(changes)
    }
}

/// A User's Trakt tokens, as Trakt hands them out.
#[derive(Clone, Serialize, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
}

/// Tokens renewed from the link whose refresh token is `linked`, kept in the Store since the
/// Server keeps only those linking gave.
#[derive(Serialize, Deserialize)]
struct Refreshed {
    linked: String,
    #[serde(flatten)]
    tokens: Tokens,
}

impl Refreshed {
    /// What the Store keeps for the User `user_id`, or `None` when it keeps none or isn't granted.
    fn kept(user_id: i64) -> Option<Self> {
        let kept = kernel::store_get(TOKENS_KEY, Some(user_id)).ok()??;
        json::from_slice(&kept).ok()
    }

    /// Keeps it for the User `user_id`, if the Admin granted the Store.
    fn keep(&self, user_id: i64) {
        if let Ok(value) = json::to_vec(self) {
            let _not_kept: Result<(), Error> = kernel::store_put(TOKENS_KEY, Some(user_id), &value);
        }
    }
}

/// When this Plugin last added a play of each title to a User's history, by the title's first id,
/// for the last [`PLAY_WINDOW_MS`]. Without the Store it is always empty, and each position report
/// past the end adds a play.
#[derive(Default, Serialize, Deserialize)]
struct Plays(BTreeMap<String, i64>);

impl Plays {
    fn kept(user_id: i64) -> Self {
        let kept = kernel::store_get(PLAYS_KEY, Some(user_id)).ok().flatten();
        kept.and_then(|kept| json::from_slice(&kept).ok())
            .unwrap_or_default()
    }

    /// Whether `change` is a new play, not one already added in the window, which it then is.
    fn is_new(&mut self, change: &SyncedWatchState) -> bool {
        let at = change.changed_at_ms;
        let added = self.0.get(&play_key(change)).copied();
        let new = added.is_none_or(|added| at.saturating_sub(added) >= PLAY_WINDOW_MS);
        if new {
            self.0.insert(play_key(change), at);
        }
        new
    }

    fn forget(&mut self, change: &SyncedWatchState) {
        self.0.remove(&play_key(change));
    }

    /// Keeps the plays still in the window for the User `user_id`, if the Admin granted the Store.
    fn keep(mut self, user_id: i64) {
        let newest = self.0.values().copied().max().unwrap_or_default();
        self.0
            .retain(|_, &mut added| newest.saturating_sub(added) < PLAY_WINDOW_MS);
        if let Ok(value) = json::to_vec(&self) {
            let _not_kept: Result<(), Error> = kernel::store_put(PLAYS_KEY, Some(user_id), &value);
        }
    }
}

/// A title as [`Plays`] keys it: its kind and the first of its ids, the one the Kernel keeps its
/// Watch State under.
fn play_key(change: &SyncedWatchState) -> String {
    let id = change.ids.first();
    let id = id.map_or_else(String::new, |id| format!("{}-{}", id.namespace, id.value));
    let kind = if change.kind == TitleRefKind::TITLE_REF_KIND_EPISODE {
        "episode"
    } else {
        "movie"
    };
    format!("{kind}:{id}")
}

/// What `/sync/history` and `/sync/history/remove` take.
#[derive(Default, Serialize)]
struct History {
    movies: Vec<Watched>,
    episodes: Vec<Watched>,
}

impl History {
    /// Adds `change`'s title, unless it is neither a Movie nor an Episode or has no TMDB or TVDB
    /// id, which Trakt can't find it by.
    fn add(&mut self, change: &SyncedWatchState, watched_at: Option<String>) {
        let Some(ids) = TraktIds::of(&change.ids) else {
            return;
        };
        let titles = match change.kind.as_known() {
            Some(TitleRefKind::TITLE_REF_KIND_MOVIE) => &mut self.movies,
            Some(TitleRefKind::TITLE_REF_KIND_EPISODE) => &mut self.episodes,
            _ => return,
        };
        titles.push(Watched { ids, watched_at });
    }
}

/// A Movie or Episode in [`History`]: an Episode by its own ids, which Trakt finds it by as well as
/// by its Show's id and numbers.
#[derive(Serialize)]
struct Watched {
    ids: TraktIds,
    #[serde(skip_serializing_if = "Option::is_none")]
    watched_at: Option<String>,
}

/// A title's ids as Trakt writes them, of those the Kernel keeps Watch State under.
#[derive(Serialize, Deserialize)]
struct TraktIds {
    #[serde(skip_serializing_if = "Option::is_none")]
    tmdb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tvdb: Option<u64>,
}

impl TraktIds {
    /// The TMDB and TVDB ids among `ids`, or `None` when it has neither.
    fn of(ids: &[TitleId]) -> Option<Self> {
        let number = |namespace| title_id(ids, namespace).and_then(|id| id.parse().ok());
        let found = Self {
            tmdb: number("tmdb"),
            tvdb: number("tvdb"),
        };
        (found.tmdb.is_some() || found.tvdb.is_some()).then_some(found)
    }

    fn title_ids(&self) -> Vec<TitleId> {
        [("tmdb", self.tmdb), ("tvdb", self.tvdb)]
            .into_iter()
            .filter_map(|(namespace, id)| {
                Some(TitleId {
                    namespace: namespace.to_owned(),
                    value: id?.to_string(),
                    ..TitleId::default()
                })
            })
            .collect()
    }
}

/// What `/oauth/device/code` answers.
#[derive(Deserialize)]
struct LinkCode {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: u32,
    interval: u32,
}

/// What `/sync/last_activities` answers, of what the Plugin reads.
#[derive(Deserialize)]
struct LastActivities {
    /// When anything in the account last changed.
    all: String,
}

/// One play `/sync/history` lists: of a Movie or of an Episode.
#[derive(Deserialize)]
struct Play {
    watched_at: String,
    movie: Option<Listed>,
    episode: Option<Listed>,
}

#[derive(Deserialize)]
struct Listed {
    ids: TraktIds,
}

impl Play {
    /// The play as a played change, or `None` when it has neither a TMDB nor a TVDB id.
    fn synced(&self) -> Option<SyncedWatchState> {
        let (kind, title) = match (&self.episode, &self.movie) {
            (Some(episode), _) => (TitleRefKind::TITLE_REF_KIND_EPISODE, episode),
            (None, Some(movie)) => (TitleRefKind::TITLE_REF_KIND_MOVIE, movie),
            (None, None) => return None,
        };
        let ids = title.ids.title_ids();
        let watched_at: Timestamp = self.watched_at.parse().ok()?;
        (!ids.is_empty()).then(|| SyncedWatchState {
            kind: kind.into(),
            ids,
            played: true,
            changed_at_ms: watched_at.as_millisecond(),
            ..SyncedWatchState::default()
        })
    }
}

#[cfg(test)]
mod fake_trakt;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
