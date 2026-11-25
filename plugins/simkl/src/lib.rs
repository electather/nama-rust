//! The Simkl Installed Plugin: a Watch sync that marks watched in the Simkl account each User
//! linked in Personal settings what they play, unmarks what they mark unplayed, and brings what
//! they watched elsewhere back. The Server builds it for wasm32 and bundles it in its binary.

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
        json::{self, Value},
    },
    kernel,
    nama_plugin_proto::nama::plugin::v1::TitleRefKind,
    required_setting, services, setting_field, title_id,
};
use serde::{Deserialize, Serialize};

/// Where Simkl's API, its PIN endpoints included, is.
const API_URL: &str = "https://api.simkl.com";

const CLIENT_ID: &str = "client_id";
const ACCESS_TOKEN: &str = "access_token";

/// Simkl's API, whose 401 means it refused the User's access token.
const SIMKL: OutsideService = OutsideService::new("Simkl").credential("access token");

/// The per-User Store entry keeping the [`Plays`] added lately.
const PLAYS_KEY: &str = "plays";

/// How long after adding a play of a title another played change of it counts as the same play.
/// The Server tells a played title again with each position report, every 10 s while it plays, so
/// a rewatch within this long isn't a play of its own, and one after it counts from its first
/// report.
const PLAY_WINDOW_MS: i64 = 6 * 60 * 60 * 1000;

const SETUP: &str = "\
1. Sign in to [Simkl](https://simkl.com/) and [create an app](https://simkl.com/settings/developer/), with `urn:ietf:wg:oauth:2.0:oob` as its redirect URI.
2. Copy its Client ID into the setting below.
3. Grant the Store, which tells each new play apart from the Server's reports while it plays on.

Each User then links their own Simkl account in Personal settings.
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

    nama_pdk::watch_sync!(super::Simkl);
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        name: "Simkl".to_owned(),
        description: "Syncs what each User watches with their own Simkl account.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        setup_markdown: SETUP.to_owned(),
        points: vec![nama_pdk::point(services::WATCH_SYNC)],
        allowed_hosts: vec!["api.simkl.com".to_owned()],
        permissions: vec![Permission::PERMISSION_STORE.into()],
        settings: vec![SettingField {
            required: true,
            ..setting_field(
                CLIENT_ID,
                "Client ID",
                "Your Simkl app's Client ID.",
                SettingKind::SETTING_KIND_SECRET,
            )
        }],
        personal_settings: vec![setting_field(
            ACCESS_TOKEN,
            "Access token",
            "Set by linking your Simkl account.",
            SettingKind::SETTING_KIND_SECRET,
        )],
        ..nama_pdk::manifest("simkl", env!("CARGO_PKG_VERSION"))
    }
}

/// Syncs each User's Watch State with their Simkl account's history.
struct Simkl;

impl WatchSync for Simkl {
    /// Marks each newly played title watched in the User's Simkl history as of when it was
    /// played, and unmarks each one marked unplayed. Position reports of an unplayed title are
    /// skipped.
    fn scrobble(asked: ScrobbleAsked) -> Result<Scrobbled, Error> {
        let account = Account::of(&asked.personal_settings)?;
        let mut plays = Plays::kept(asked.user_id)?;
        let (mut watched, mut unwatched) = (History::default(), History::default());
        for change in &asked.changes {
            // The contract marks a title unplayed at 0 of 0; any other unplayed change is a
            // position report.
            let marked_unplayed =
                !change.played && change.position_ms == 0 && change.duration_ms == 0;
            if change.played && plays.is_new(change) {
                watched.add(change, rfc3339(change.changed_at_ms));
            } else if marked_unplayed {
                plays.forget(change);
                unwatched.add(change, None);
            }
        }
        account.post_history("/sync/history", &watched)?;
        // Kept before removing, so a failed removal doesn't make the next report add the plays
        // just added again.
        plays.keep(asked.user_id)?;
        account.post_history("/sync/history/remove", &unwatched)?;
        Ok(Scrobbled::default())
    }

    /// What the User watched since the last import, everything on the first; the cursor is when
    /// Simkl last saw any of their activity, so an unchanged account costs one ask. Simkl keeps
    /// no record of what was unmarked, so marking unplayed there doesn't come back.
    fn import(asked: ImportAsked) -> Result<Imported, Error> {
        let account = Account::of(&asked.personal_settings)?;
        let activities: Activities = account.send("GET", "/sync/activities", None)?.json()?;
        // Simkl's docs don't promise `all` for an account with no activity yet.
        let cursor = activities.all.unwrap_or_default();
        let changes = if cursor == asked.since {
            Vec::new()
        } else {
            account.watched_since(&asked.since)?
        };
        Ok(Imported {
            changes,
            cursor,
            ..Imported::default()
        })
    }

    /// Asks Simkl for a PIN, which is also what polling it goes by: Simkl's own device code is
    /// the same placeholder for every link.
    fn start_link(_asked: StartLinkAsked) -> Result<LinkStarted, Error> {
        let answer = SIMKL.send(&api_request("GET", "/oauth/pin")?, None::<&str>)?;
        let pin: LinkPin = SIMKL.accepted(answer)?.json()?;
        Ok(LinkStarted {
            device_code: pin.user_code.clone(),
            user_code: pin.user_code,
            verification_url: pin.verification_url,
            expires_in_seconds: pin.expires_in,
            interval_seconds: pin.interval,
            ..LinkStarted::default()
        })
    }

    fn poll_link(asked: PollLinkAsked) -> Result<LinkPolled, Error> {
        let expired = LinkPolled {
            expired: true,
            ..LinkPolled::default()
        };
        // The User's client hands the code back, and it goes into the path.
        let pin = asked.device_code;
        if pin.is_empty() || !pin.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Ok(expired);
        }
        let answer = SIMKL.send(
            &api_request("GET", &format!("/oauth/pin/{pin}"))?,
            None::<&str>,
        )?;
        // Polled too fast: the User's client asks again at the next interval.
        if answer.status_code() == 429 {
            return Ok(LinkPolled::default());
        }
        let polled: PinPolled = SIMKL.accepted(answer)?.json()?;
        Ok(match polled {
            PinPolled {
                access_token: Some(token),
                ..
            } if !token.is_empty() => LinkPolled {
                personal_settings: std::iter::once((ACCESS_TOKEN.to_owned(), token)).collect(),
                ..LinkPolled::default()
            },
            // Simkl answers a gone or unknown PIN with a new one.
            PinPolled {
                device_code: Some(_),
                ..
            } => expired,
            _ => LinkPolled::default(),
        })
    }
}

/// A request to Simkl's API at `path`, naming the Admin's app as Simkl requires of every request.
fn api_request(method: &str, path: &str) -> Result<HttpRequest, Error> {
    let client_id = required_setting(CLIENT_ID)?;
    let joiner = if path.contains('?') { '&' } else { '?' };
    let version = env!("CARGO_PKG_VERSION");
    let url =
        format!("{API_URL}{path}{joiner}client_id={client_id}&app-name=nama&app-version={version}");
    Ok(HttpRequest::new(url)
        .with_method(method)
        .with_header("Content-Type", "application/json")
        .with_header("User-Agent", format!("nama/{version}")))
}

/// `ms` since the Unix epoch as Simkl reads a time, or `None`, meaning now, if out of range.
fn rfc3339(ms: i64) -> Option<String> {
    Timestamp::from_millisecond(ms)
        .ok()
        .map(|at| at.to_string())
}

/// A User's linked Simkl account, by the access token its requests go with. Simkl hands out no
/// refresh token: its access tokens last years, and one it refuses was revoked.
struct Account {
    access_token: String,
}

impl Account {
    /// The account the User linked, by the token their Personal settings hold.
    fn of<S: BuildHasher>(personal_settings: &HashMap<String, String, S>) -> Result<Self, Error> {
        let access_token = (personal_settings.get(ACCESS_TOKEN))
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                Error::msg("the User hasn't linked a Simkl account in Personal settings")
            })?;
        Ok(Self {
            access_token: access_token.clone(),
        })
    }

    /// Simkl's 2xx answer to `method` `path` with `body`. A 401 asks the User to link again.
    fn send(&self, method: &str, path: &str, body: Option<&str>) -> Result<Response, Error> {
        let bearer = format!("Bearer {}", self.access_token);
        let request = api_request(method, path)?.with_header("Authorization", bearer);
        let answer = SIMKL.send(&request, body)?;
        if answer.status_code() == 401 {
            return Err(Error::msg(
                "Simkl no longer takes this User's link; they link Simkl again in Personal \
                 settings",
            ));
        }
        Ok(SIMKL.accepted(answer)?)
    }

    /// Every Movie and Episode watched that changed after `since`, Simkl's own time as
    /// `/sync/activities` gave it, or ever when it's empty, skipping those with no TMDB or TVDB
    /// id.
    fn watched_since(&self, since: &str) -> Result<Vec<SyncedWatchState>, Error> {
        let date_from = if since.is_empty() {
            String::new()
        } else {
            format!("&date_from={since}")
        };
        let path = format!(
            "/sync/all-items?extended=full&episode_watched_at=yes&episode_tvdb_id=yes{date_from}"
        );
        // Simkl answers `null` when nothing changed.
        let items: Option<AllItems> = self.send("GET", &path, None)?.json()?;
        Ok(items.unwrap_or_default().watched())
    }

    /// POSTs `history` to `path`, unless it is empty.
    fn post_history(&self, path: &str, history: &History) -> Result<(), Error> {
        if history.movies.is_empty() && history.episodes.is_empty() {
            return Ok(());
        }
        self.send("POST", path, Some(&json::to_string(history)?))
            .map(drop)
    }
}

/// When this Plugin last added a play of each title to a User's history, by the title's first id,
/// for the last [`PLAY_WINDOW_MS`].
#[derive(Default, Serialize, Deserialize)]
struct Plays(BTreeMap<String, i64>);

impl Plays {
    /// What the Store keeps for the User `user_id`, empty if it keeps nothing readable.
    ///
    /// # Errors
    ///
    /// When the Admin didn't grant the Store: without it each position report of a played title
    /// would be sent again.
    fn kept(user_id: i64) -> Result<Self, Error> {
        let kept = kernel::store_get(PLAYS_KEY, Some(user_id)).map_err(|_refused| {
            Error::msg("Simkl needs the Store, to tell a new play from the Server's reports")
        })?;
        Ok(kept
            .and_then(|kept| json::from_slice(&kept).ok())
            .unwrap_or_default())
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

    /// Keeps the plays still in the window for the User `user_id`.
    fn keep(mut self, user_id: i64) -> Result<(), Error> {
        let newest = self.0.values().copied().max().unwrap_or_default();
        self.0
            .retain(|_, &mut added| newest.saturating_sub(added) < PLAY_WINDOW_MS);
        kernel::store_put(PLAYS_KEY, Some(user_id), &json::to_vec(&self)?)
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
    /// Adds `change`'s title, unless it is neither a Movie nor an Episode or has no id Simkl can
    /// find it by: a Movie's TMDB or TVDB id, or an Episode's TVDB id, the only Episode id Simkl
    /// takes.
    fn add(&mut self, change: &SyncedWatchState, watched_at: Option<String>) {
        let number = |namespace| title_id(&change.ids, namespace).and_then(|id| id.parse().ok());
        let (titles, ids) = match change.kind.as_known() {
            Some(TitleRefKind::TITLE_REF_KIND_MOVIE) => (
                &mut self.movies,
                SimklIds {
                    tmdb: number("tmdb"),
                    tvdb: number("tvdb"),
                },
            ),
            Some(TitleRefKind::TITLE_REF_KIND_EPISODE) => (
                &mut self.episodes,
                SimklIds {
                    tmdb: None,
                    tvdb: number("tvdb"),
                },
            ),
            _ => return,
        };
        if ids.tmdb.is_some() || ids.tvdb.is_some() {
            titles.push(Watched { ids, watched_at });
        }
    }
}

/// A Movie or Episode in [`History`]: an Episode by its own TVDB id, which Simkl matches it by
/// ahead of its Show and number.
#[derive(Serialize)]
struct Watched {
    ids: SimklIds,
    #[serde(skip_serializing_if = "Option::is_none")]
    watched_at: Option<String>,
}

/// A title's ids as Simkl takes them, of those the Kernel keeps Watch State under.
#[derive(Serialize)]
struct SimklIds {
    #[serde(skip_serializing_if = "Option::is_none")]
    tmdb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tvdb: Option<u64>,
}

/// What `/sync/activities` answers, of what the Plugin reads.
#[derive(Deserialize)]
struct Activities {
    /// When anything in the account last changed.
    all: Option<String>,
}

/// What `/sync/all-items` answers, of what the Plugin reads.
#[derive(Default, Deserialize)]
#[serde(default)]
struct AllItems {
    movies: Vec<ListedMovie>,
    shows: Vec<ListedShow>,
    anime: Vec<ListedShow>,
}

impl AllItems {
    /// Each completed Movie and each watched Episode as a played change.
    fn watched(&self) -> Vec<SyncedWatchState> {
        let movies = (self.movies.iter())
            .filter(|listed| listed.status == "completed")
            .filter_map(|listed| {
                let ids = &listed.movie.ids;
                let ids = [
                    ("tmdb", numeric_id(ids, "tmdb")),
                    ("tvdb", numeric_id(ids, "tvdb")),
                ];
                played(
                    TitleRefKind::TITLE_REF_KIND_MOVIE,
                    &ids,
                    listed.last_watched_at.as_deref(),
                )
            });
        let episodes = (self.shows.iter().chain(&self.anime))
            .flat_map(|show| &show.seasons)
            .flat_map(|season| &season.episodes)
            .filter_map(|episode| {
                let ids = [("tvdb", numeric_id(&episode.ids, "tvdb_id"))];
                played(
                    TitleRefKind::TITLE_REF_KIND_EPISODE,
                    &ids,
                    episode.watched_at.as_deref(),
                )
            });
        movies.chain(episodes).collect()
    }
}

/// A title of `kind` with `ids` played at `watched_at`, or `None` without an id or a time.
fn played(
    kind: TitleRefKind,
    ids: &[(&str, Option<String>)],
    watched_at: Option<&str>,
) -> Option<SyncedWatchState> {
    let ids: Vec<TitleId> = (ids.iter())
        .filter_map(|(namespace, value)| {
            Some(TitleId {
                namespace: (*namespace).to_owned(),
                value: value.clone()?,
                ..TitleId::default()
            })
        })
        .collect();
    let watched_at: Timestamp = watched_at?.parse().ok()?;
    (!ids.is_empty()).then(|| SyncedWatchState {
        kind: kind.into(),
        ids,
        played: true,
        changed_at_ms: watched_at.as_millisecond(),
        ..SyncedWatchState::default()
    })
}

/// The id `key` of `ids` when it is a number, which Simkl writes as a string or a number.
fn numeric_id(ids: &HashMap<String, Value>, key: &str) -> Option<String> {
    let value = ids.get(key)?;
    let number: u64 = value.as_u64().or_else(|| value.as_str()?.parse().ok())?;
    Some(number.to_string())
}

#[derive(Deserialize)]
struct ListedMovie {
    status: String,
    last_watched_at: Option<String>,
    movie: Listed,
}

#[derive(Deserialize)]
struct Listed {
    ids: HashMap<String, Value>,
}

#[derive(Deserialize)]
struct ListedShow {
    #[serde(default)]
    seasons: Vec<ListedSeason>,
}

#[derive(Deserialize)]
struct ListedSeason {
    #[serde(default)]
    episodes: Vec<ListedEpisode>,
}

#[derive(Deserialize)]
struct ListedEpisode {
    watched_at: Option<String>,
    #[serde(default)]
    ids: HashMap<String, Value>,
}

/// What `/oauth/pin` answers.
#[derive(Deserialize)]
struct LinkPin {
    user_code: String,
    verification_url: String,
    expires_in: u32,
    interval: u32,
}

/// What `/oauth/pin/{pin}` answers, of what the Plugin reads.
#[derive(Deserialize)]
struct PinPolled {
    access_token: Option<String>,
    device_code: Option<String>,
}

#[cfg(test)]
mod fake_simkl;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
