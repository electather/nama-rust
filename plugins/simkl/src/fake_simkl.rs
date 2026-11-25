//! An in-process stand-in for Simkl's API, so the tests need no network and no account. It knows
//! one API app, one PIN and one User's watched titles, and checks each request's app and token as
//! Simkl does.

use jiff::Timestamp;
use nama_pdk::{
    Response,
    extism_pdk::{
        HttpRequest,
        json::{self, Value, json},
    },
};

/// The API app's Client ID the fake knows.
pub(super) const CLIENT_ID: &str = "app-client-id";

/// The one code the fake hands out for the User to enter.
pub(super) const USER_CODE: &str = "A1B2C";

/// The one access token linking gives.
pub(super) const ACCESS_TOKEN: &str = "access-1";

/// Where the PIN stands.
pub(super) enum Pin {
    /// The User hasn't entered it yet.
    Waiting,
    /// The User entered it, so polling it links their account.
    Entered,
    /// It ran out before the User entered it.
    Expired,
}

/// Simkl's API in memory.
pub(super) struct FakeSimkl {
    /// Where the PIN it handed out stands.
    pub(super) pin: Pin,
    /// Answers the User's token with a 401, as once they revoke the app.
    pub(super) revoked: bool,
    /// What the User watched, oldest first: `{ "kind", "ids", "watched_at" }`.
    pub(super) watched: Vec<Value>,
    /// When anything in the User's account last changed, as `/sync/activities` says.
    pub(super) last_activity: String,
    /// Answers every request with a 429 when set.
    pub(super) limiting: bool,
}

impl FakeSimkl {
    /// A User who has watched nothing yet and hasn't entered the PIN.
    pub(super) fn new() -> Self {
        Self {
            pin: Pin::Waiting,
            revoked: false,
            watched: Vec::new(),
            last_activity: "2026-11-01T00:00:00Z".to_owned(),
            limiting: false,
        }
    }

    /// Marks the `kind` (`movie` or `episode`) with `ids` watched at `watched_at`, as watching it
    /// elsewhere does.
    pub(super) fn watch(&mut self, kind: &str, ids: &Value, watched_at: &str) {
        self.watched
            .push(json!({ "kind": kind, "ids": ids.clone(), "watched_at": watched_at }));
        self.last_activity = watched_at.to_owned();
    }

    /// Simkl's answer to `request` with `body`.
    pub(super) fn answer(&mut self, request: &HttpRequest, body: &[u8]) -> Response {
        if self.limiting {
            return Response::new(429, "").with_header("Retry-After", "30");
        }
        let Some(url) = request.url.strip_prefix("https://api.simkl.com") else {
            return Response::new(404, "");
        };
        let (path, query) = url.split_once('?').unwrap_or((url, ""));
        let param = |name: &str| {
            (query.split('&'))
                .filter_map(|pair| pair.split_once('='))
                .find_map(|(key, value)| (key == name).then_some(value))
        };
        let app = param("client_id") == Some(CLIENT_ID)
            && param("app-name").is_some()
            && param("app-version").is_some()
            && request.headers.contains_key("User-Agent");
        if !app {
            return Response::new(412, r#"{"error":"client_id_failed"}"#);
        }
        let body: Value = json::from_slice(body).unwrap_or_default();
        let method = request.method.as_deref().unwrap_or("GET");
        let bearer = format!("Bearer {ACCESS_TOKEN}");
        if let Some(user_code) = path.strip_prefix("/oauth/pin/") {
            return self.polled(user_code);
        }
        match (method, path) {
            ("GET", "/oauth/pin") => Self::code(),
            _ if self.revoked
                || request.headers.get("Authorization").map(String::as_str) != Some(&bearer) =>
            {
                Response::new(401, r#"{"error":"user_token_failed"}"#)
            }
            ("GET", "/sync/activities") => ok(&json!({
                "all": self.last_activity,
                "movies": { "all": self.last_activity },
                "tv_shows": { "all": self.last_activity },
            })),
            ("GET", "/sync/all-items") => self.all_items(param("date_from")),
            ("POST", "/sync/history") => self.added(&body),
            ("POST", "/sync/history/remove") => self.removed(&body),
            _ => Response::new(404, ""),
        }
    }

    fn code() -> Response {
        ok(&json!({
            "result": "OK",
            "device_code": "DEVICE_CODE",
            "user_code": USER_CODE,
            "verification_uri": "https://simkl.com/pin",
            "verification_url": "https://simkl.com/pin",
            "expires_in": 900,
            "interval": 5,
        }))
    }

    /// As Simkl answers a poll of `user_code`: an unknown or gone code gets a new one.
    fn polled(&self, user_code: &str) -> Response {
        match (&self.pin, user_code == USER_CODE) {
            (Pin::Waiting, true) => {
                ok(&json!({ "result": "KO", "message": "Authorization pending" }))
            }
            (Pin::Entered, true) => ok(&json!({ "result": "OK", "access_token": ACCESS_TOKEN })),
            (Pin::Expired, _) | (_, false) => Self::code(),
        }
    }

    /// What was watched after `date_from`, everything without it, each Episode as a Show of its
    /// own, as `extended=full&episode_watched_at=yes&episode_tvdb_id=yes` lists them.
    fn all_items(&self, date_from: Option<&str>) -> Response {
        let after = date_from.map(|at| at.parse::<Timestamp>().expect("an RFC 3339 time"));
        let watched_at = |title: &Value| title["watched_at"].as_str()?.parse::<Timestamp>().ok();
        let since = (self.watched.iter())
            .filter(|&title| after.is_none_or(|after| watched_at(title) > Some(after)));
        let (mut movies, mut shows) = (Vec::new(), Vec::new());
        for title in since {
            if title["kind"] == "movie" {
                movies.push(json!({
                    "status": "completed",
                    "last_watched_at": title["watched_at"],
                    "movie": { "title": "A Movie", "ids": title["ids"] },
                }));
            } else {
                shows.push(json!({
                    "status": "watching",
                    "last_watched_at": title["watched_at"],
                    "show": { "title": "A Show", "ids": { "simkl": 1 } },
                    "seasons": [{ "number": 1, "episodes": [{
                        "number": 1,
                        "watched_at": title["watched_at"],
                        "ids": { "tvdb_id": title["ids"]["tvdb"] },
                    }] }],
                }));
            }
        }
        if movies.is_empty() && shows.is_empty() {
            return ok(&Value::Null);
        }
        ok(&json!({ "movies": movies, "shows": shows }))
    }

    fn added(&mut self, body: &Value) -> Response {
        for (list, kind) in [("movies", "movie"), ("episodes", "episode")] {
            for title in body[list].as_array().into_iter().flatten() {
                let watched_at = title["watched_at"]
                    .as_str()
                    .unwrap_or("2026-11-25T00:00:00Z");
                self.watch(kind, &title["ids"], watched_at);
            }
        }
        Response::new(201, json!({ "added": {}, "not_found": {} }).to_string())
    }

    fn removed(&mut self, body: &Value) -> Response {
        for list in ["movies", "episodes"] {
            for title in body[list].as_array().into_iter().flatten() {
                self.watched
                    .retain(|watched| watched["ids"] != title["ids"]);
            }
        }
        ok(&json!({ "deleted": {}, "not_found": {} }))
    }
}

fn ok(body: &Value) -> Response {
    Response::new(200, body.to_string())
}
