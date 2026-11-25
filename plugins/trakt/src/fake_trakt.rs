//! An in-process stand-in for Trakt's API, so the tests need no network and no account. It knows
//! one API app, one device code and one User's history, and checks each request's app, version
//! and token as Trakt does.

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

/// The API app's Client Secret the fake knows.
pub(super) const CLIENT_SECRET: &str = "app-client-secret";

/// The one device code the fake hands out.
pub(super) const DEVICE_CODE: &str = "device-code-1";

/// Where a device code stands.
pub(super) enum Code {
    /// The User hasn't entered it yet.
    Waiting,
    /// The User entered it, so polling it links their account.
    Entered,
    /// It ran out before the User entered it.
    Expired,
}

/// Trakt's API in memory.
pub(super) struct FakeTrakt {
    /// Where the device code it handed out stands.
    pub(super) code: Code,
    /// The one access token it takes, unless it expired.
    pub(super) access_token: String,
    access_token_expired: bool,
    /// The one refresh token that renews it, spent once used.
    pub(super) refresh_token: String,
    /// How many token pairs it has issued, which names the next.
    issued: u32,
    /// The User's history, newest first, each play as `/sync/history` lists it.
    pub(super) history: Vec<Value>,
    /// When anything in the User's account last changed, as `/sync/last_activities` says.
    pub(super) last_activity: String,
    /// How many plays each page of `/sync/history` holds at most.
    pub(super) page_size: usize,
    /// Answers every request with a 429 when set.
    pub(super) limiting: bool,
}

impl FakeTrakt {
    /// A User with no history yet, who hasn't entered the device code.
    pub(super) fn new() -> Self {
        Self {
            code: Code::Waiting,
            access_token: String::new(),
            access_token_expired: false,
            refresh_token: String::new(),
            issued: 0,
            history: Vec::new(),
            last_activity: "2026-11-01T00:00:00.000Z".to_owned(),
            page_size: 100,
            limiting: false,
        }
    }

    /// Issues the tokens a link gives, as if the User had entered the code.
    pub(super) fn linked() -> Self {
        let mut trakt = Self::new();
        trakt.issue();
        trakt
    }

    /// Lets the access token expire, so Trakt answers it with a 401.
    pub(super) fn expire_access_token(&mut self) {
        self.access_token_expired = true;
    }

    /// Adds a play of the `kind` (`movie` or `episode`) with `ids` at `watched_at` to the history,
    /// as watching it elsewhere does.
    pub(super) fn watch(&mut self, kind: &str, ids: &Value, watched_at: &str) {
        self.history.insert(
            0,
            json!({ "watched_at": watched_at, "type": kind, kind: { "ids": ids.clone() } }),
        );
        self.last_activity = watched_at.to_owned();
    }

    /// Trakt's answer to `request` with `body`.
    pub(super) fn answer(&mut self, request: &HttpRequest, body: &[u8]) -> Response {
        if self.limiting {
            return Response::new(429, "").with_header("Retry-After", "30");
        }
        let header = |name| request.headers.get(name).map(String::as_str);
        if header("trakt-api-key") != Some(CLIENT_ID) || header("trakt-api-version") != Some("2") {
            return Response::new(403, "");
        }
        let Some(path) = request.url.strip_prefix("https://api.trakt.tv") else {
            return Response::new(404, "");
        };
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let body: Value = json::from_slice(body).unwrap_or_default();
        let method = request.method.as_deref().unwrap_or("GET");
        match (method, path) {
            ("POST", "/oauth/device/code") => Self::device_code(&body),
            ("POST", "/oauth/device/token") => self.device_token(&body),
            ("POST", "/oauth/token") => self.refreshed(&body),
            _ if self.access_token_expired
                || header("Authorization") != Some(&format!("Bearer {}", self.access_token)) =>
            {
                Response::new(401, "")
            }
            ("GET", "/sync/last_activities") => {
                ok(&json!({ "all": self.last_activity, "movies": {}, "episodes": {} }))
            }
            ("GET", "/sync/history") => self.listed(query),
            ("POST", "/sync/history") => self.added(&body),
            ("POST", "/sync/history/remove") => self.removed(&body),
            _ => Response::new(404, ""),
        }
    }

    fn device_code(body: &Value) -> Response {
        if body["client_id"] != CLIENT_ID {
            return Response::new(401, "");
        }
        ok(&json!({
            "device_code": DEVICE_CODE,
            "user_code": "5055CC52",
            "verification_url": "https://trakt.tv/activate",
            "expires_in": 600,
            "interval": 5,
        }))
    }

    fn device_token(&mut self, body: &Value) -> Response {
        if body["client_id"] != CLIENT_ID || body["client_secret"] != CLIENT_SECRET {
            return Response::new(401, "");
        }
        if body["code"] != DEVICE_CODE {
            return Response::new(404, "");
        }
        match self.code {
            Code::Waiting => Response::new(400, ""),
            Code::Entered => self.issue(),
            Code::Expired => Response::new(410, ""),
        }
    }

    fn refreshed(&mut self, body: &Value) -> Response {
        let app = body["client_id"] == CLIENT_ID && body["client_secret"] == CLIENT_SECRET;
        let grant = body["grant_type"] == "refresh_token"
            && body["redirect_uri"] == "urn:ietf:wg:oauth:2.0:oob"
            && body["refresh_token"] == self.refresh_token.as_str();
        if !(app && grant) {
            return Response::new(401, r#"{"error":"invalid_grant"}"#);
        }
        self.issue()
    }

    /// Issues a new token pair, spending the last.
    fn issue(&mut self) -> Response {
        self.issued += 1;
        self.access_token_expired = false;
        self.access_token = format!("access-{}", self.issued);
        self.refresh_token = format!("refresh-{}", self.issued);
        ok(&json!({
            "access_token": self.access_token,
            "token_type": "bearer",
            "expires_in": 86400,
            "refresh_token": self.refresh_token,
            "scope": "public",
            "created_at": 1_790_000_000,
        }))
    }

    fn listed(&self, query: &str) -> Response {
        let param = |name: &str| {
            (query.split('&'))
                .filter_map(|pair| pair.split_once('='))
                .find_map(|(key, value)| (key == name).then_some(value))
        };
        let number = |name| param(name).and_then(|value| value.parse::<usize>().ok());
        let start_at =
            param("start_at").map(|at| at.parse::<Timestamp>().expect("an RFC 3339 time"));
        let watched_at = |play: &Value| play["watched_at"].as_str()?.parse::<Timestamp>().ok();
        let since: Vec<&Value> = (self.history.iter())
            .filter(|&play| start_at.is_none_or(|start_at| watched_at(play) >= Some(start_at)))
            .collect();
        let limit = number("limit").unwrap_or(10).min(self.page_size);
        let page = number("page").unwrap_or(1);
        let pages = since.len().div_ceil(limit).max(1);
        let listed: Vec<_> = since
            .into_iter()
            .skip((page - 1) * limit)
            .take(limit)
            .collect();
        ok(&json!(listed)).with_header("X-Pagination-Page-Count", &pages.to_string())
    }

    fn added(&mut self, body: &Value) -> Response {
        for (list, kind) in [("movies", "movie"), ("episodes", "episode")] {
            for title in body[list].as_array().into_iter().flatten() {
                let watched_at = title["watched_at"]
                    .as_str()
                    .unwrap_or("2026-11-25T00:00:00.000Z");
                self.watch(kind, &title["ids"], watched_at);
            }
        }
        ok(&json!({ "added": {} }))
    }

    fn removed(&mut self, body: &Value) -> Response {
        for (list, kind) in [("movies", "movie"), ("episodes", "episode")] {
            for title in body[list].as_array().into_iter().flatten() {
                self.history
                    .retain(|play| play[kind]["ids"] != title["ids"]);
            }
        }
        ok(&json!({ "deleted": {} }))
    }
}

fn ok(body: &Value) -> Response {
    Response::new(200, body.to_string())
}
