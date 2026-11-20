//! An in-process stand-in for OpenSubtitles' REST API v1, so the tests need no network and no
//! account. It refuses a request without [`API_KEY`] or a User-Agent, a login other than
//! [`USERNAME`] and [`PASSWORD`], and a search whose parameters or languages aren't sorted and
//! lowercase, which OpenSubtitles answers with a redirect. It panics at a request OpenSubtitles has
//! no answer for.

use nama_pdk::{
    Response,
    extism_pdk::{
        HttpRequest,
        json::{self, Value, json},
    },
};
use url::Url;

/// The API key the fake takes.
pub(super) const API_KEY: &str = "fake-api-key";

/// The account the fake logs in.
pub(super) const USERNAME: &str = "admin";

/// The account's password.
pub(super) const PASSWORD: &str = "hunter2";

/// One subtitle the fake serves.
pub(super) struct FakeSubtitle {
    pub(super) file_id: u64,
    pub(super) language: &'static str,
    pub(super) release: &'static str,
    pub(super) sdh: bool,
    /// How many files it comes in, one per CD.
    pub(super) files: u64,
    /// The search parameters, but `languages`, it's found by, sorted, such as
    /// `[("imdb_id", "78748"), ("type", "movie")]`.
    pub(super) found_by: Vec<(&'static str, &'static str)>,
}

impl FakeSubtitle {
    /// A one-file English subtitle for the Movie with the IMDb id 78748.
    pub(super) fn new(file_id: u64, release: &'static str) -> Self {
        Self {
            file_id,
            language: "en",
            release,
            sdh: false,
            files: 1,
            found_by: vec![("imdb_id", "78748"), ("type", "movie")],
        }
    }
}

/// OpenSubtitles in memory.
pub(super) struct FakeOpenSubtitles {
    pub(super) subtitles: Vec<FakeSubtitle>,
    /// Downloads the account has left today.
    pub(super) remaining: u32,
    /// Whether it answers every request as past its rate limit.
    pub(super) rate_limited: bool,
    /// The API host a login answers, which alone takes the account's downloads.
    pub(super) base_url: &'static str,
    /// Whether the file host refuses every link, as it does one that expired.
    pub(super) links_expired: bool,
    /// The token a login answers, and the only one a download takes.
    pub(super) token: &'static str,
}

impl Default for FakeOpenSubtitles {
    fn default() -> Self {
        Self {
            subtitles: Vec::new(),
            remaining: 20,
            rate_limited: false,
            base_url: "api.opensubtitles.com",
            links_expired: false,
            token: "fake-token",
        }
    }
}

impl FakeOpenSubtitles {
    /// OpenSubtitles' answer to `request` with `body`.
    pub(super) fn answer(&mut self, request: &HttpRequest, body: &[u8]) -> Response {
        let url = Url::parse(&request.url).expect("the Plugin sends a URL");
        if url.host_str() == Some("www.opensubtitles.com") {
            return self.file(&url);
        }
        let host = url.host_str();
        assert!(matches!(
            host,
            Some("api.opensubtitles.com" | "vip-api.opensubtitles.com")
        ));
        let header = |name: &str| request.headers.get(name).map(String::as_str);
        if header("Api-Key") != Some(API_KEY) || header("User-Agent").is_none_or(str::is_empty) {
            return answer(
                403,
                &json!({ "message": "You cannot consume this service" }),
            );
        }
        if self.rate_limited {
            return answer(
                429,
                &json!({ "message": "Throttle limit reached. Retry later." }),
            )
            .with_header("Retry-After", "7");
        }
        let method = request.method.as_deref().unwrap_or("GET");
        let body: Value = json::from_slice(body).unwrap_or_default();
        match (method, url.path()) {
            ("POST", "/api/v1/login") => self.login(&body),
            ("GET", "/api/v1/subtitles") => self.search(&url),
            ("POST", "/api/v1/download")
                if host == Some(self.base_url)
                    && header("Authorization").and_then(|auth| auth.strip_prefix("Bearer "))
                        == Some(self.token) =>
            {
                self.download(&body)
            }
            ("POST", "/api/v1/download") => answer(401, &json!({ "message": "Unauthorized" })),
            (method, path) => panic!("OpenSubtitles has no {method} {path}"),
        }
    }

    fn search(&self, url: &Url) -> Response {
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let sorted = pairs.is_sorted_by(|a, b| a.0 < b.0);
        if !sorted
            || pairs
                .iter()
                .any(|(_, value)| *value != value.to_lowercase())
        {
            return answer(301, &json!({}));
        }
        let languages: Vec<&str> = (pairs.iter())
            .find(|(key, _)| key == "languages")
            .map_or_else(Vec::new, |(_, languages)| languages.split(',').collect());
        if !languages.is_sorted() {
            return answer(301, &json!({}));
        }
        let asked: Vec<(&str, &str)> = (pairs.iter())
            .filter(|(key, _)| key != "languages")
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let data: Vec<Value> = (self.subtitles.iter())
            .filter(|subtitle| subtitle.found_by == asked)
            .filter(|subtitle| languages.contains(&subtitle.language.to_lowercase().as_str()))
            .map(|subtitle| {
                let files: Vec<Value> = (0..subtitle.files)
                    .map(|cd| json!({ "file_id": subtitle.file_id + cd, "cd_number": cd + 1 }))
                    .collect();
                json!({
                    "id": subtitle.file_id.to_string(),
                    "type": "subtitle",
                    "attributes": {
                        "language": subtitle.language,
                        "release": subtitle.release,
                        "hearing_impaired": subtitle.sdh,
                        "files": files,
                    },
                })
            })
            .collect();
        answer(
            200,
            &json!({ "total_count": data.len(), "page": 1, "data": data }),
        )
    }

    fn download(&mut self, body: &Value) -> Response {
        let Some(subtitle) = (self.subtitles.iter())
            .find(|subtitle| Some(subtitle.file_id) == body["file_id"].as_u64())
        else {
            return answer(404, &json!({ "message": "File not found" }));
        };
        if self.remaining == 0 {
            return answer(
                406,
                &json!({
                    "requests": 21,
                    "remaining": 0,
                    "message": "You have downloaded your allowed 20 subtitles for 24h.",
                    "reset_time": "3 hours and 12 minutes",
                    "reset_time_utc": "2026-11-21T02:00:00.000Z",
                }),
            );
        }
        self.remaining -= 1;
        let file_name = format!("{}.srt", subtitle.release);
        answer(
            200,
            &json!({
                "link": format!("https://www.opensubtitles.com/download/{}/{file_name}", subtitle.file_id),
                "file_name": file_name,
                "remaining": self.remaining,
            }),
        )
    }

    fn login(&self, body: &Value) -> Response {
        if body["username"] == USERNAME && body["password"] == PASSWORD {
            answer(
                200,
                &json!({
                    "user": { "allowed_downloads": 20, "vip": false },
                    "base_url": self.base_url,
                    "token": self.token,
                    "status": 200,
                }),
            )
        } else {
            answer(
                401,
                &json!({ "message": "Error, invalid username/password", "status": 401 }),
            )
        }
    }

    /// The file a download's link points at.
    fn file(&self, url: &Url) -> Response {
        if self.links_expired {
            return Response::new(403, "");
        }
        let segments: Vec<_> = url.path_segments().into_iter().flatten().collect();
        let found = match segments.as_slice() {
            ["download", id, _] => {
                (self.subtitles.iter()).find(|subtitle| subtitle.file_id.to_string() == *id)
            }
            _ => None,
        };
        match found {
            Some(subtitle) => Response::new(200, srt(subtitle.release)),
            None => Response::new(404, ""),
        }
    }
}

/// The file the fake serves for the subtitle `release`.
pub(super) fn srt(release: &str) -> String {
    format!("1\n00:00:01,000 --> 00:00:02,000\n{release}\n")
}

fn answer(status: u16, body: &Value) -> Response {
    Response::new(status, json::to_vec(body).unwrap_or_default())
}
