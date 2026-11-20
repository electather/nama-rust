//! The OpenSubtitles.com Installed Plugin: a subtitle source searching OpenSubtitles' REST API v1
//! for a Version's subtitles and downloading one with the Admin's account. The Server builds it for
//! wasm32 and bundles it in its binary.

use std::fmt;

use nama_pdk::{
    CheckedSettings, Manifest, Permission, Response, SubtitleDownload, SubtitleFile,
    SubtitleResult, SubtitleResults, SubtitleSearch, SubtitleSource, TitleKind,
    extism_pdk::{
        Error, HttpRequest, Json,
        json::{self, json},
    },
    kernel,
    nama_plugin_proto::nama::plugin::v1::{SettingField, SettingKind},
    services,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

/// Where OpenSubtitles' REST API is.
const API_URL: &str = "https://api.opensubtitles.com/api/v1/";

const API_KEY: &str = "api_key";
const USERNAME: &str = "username";
const PASSWORD: &str = "password";

/// The Plugin-wide Store entry keeping the last [`LoggedIn`].
const LOGIN_KEY: &str = "login";

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, CheckedSettings, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        Ok(Buffa(super::check_settings()?))
    }

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }
}

/// What `check_settings()` returns: it logs in, so testing the settings reports a key or account
/// OpenSubtitles refuses.
fn check_settings() -> Result<CheckedSettings, Error> {
    login()?;
    Ok(CheckedSettings::default())
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let setting = |key: &str, label: &str, description: &str, kind: SettingKind| SettingField {
        key: key.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: kind.into(),
        required: true,
        ..SettingField::default()
    };
    Manifest {
        name: "OpenSubtitles.com".to_owned(),
        description: "Finds and downloads subtitles from OpenSubtitles.com.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::SUBTITLE_SOURCE)],
        // Download links point at another host of OpenSubtitles' than its API.
        allowed_hosts: vec!["*.opensubtitles.com".to_owned()],
        // To keep a login between downloads; without it, each download logs in.
        permissions: vec![Permission::PERMISSION_STORE.into()],
        settings: vec![
            setting(
                API_KEY,
                "API key",
                "The API key of a consumer you create under API consumers in your \
                 OpenSubtitles.com profile.",
                SettingKind::SETTING_KIND_SECRET,
            ),
            setting(
                USERNAME,
                "Username",
                "The OpenSubtitles.com account every download counts against.",
                SettingKind::SETTING_KIND_TEXT,
            ),
            setting(
                PASSWORD,
                "Password",
                "That account's password.",
                SettingKind::SETTING_KIND_SECRET,
            ),
        ],
        ..nama_pdk::manifest("opensubtitles", env!("CARGO_PKG_VERSION"))
    }
}

/// Serves subtitles from OpenSubtitles.
struct OpenSubtitles;

impl SubtitleSource for OpenSubtitles {
    fn search(search: SubtitleSearch) -> Result<SubtitleResults, Error> {
        let mut url = Url::parse(API_URL)?.join("subtitles")?;
        url.query_pairs_mut().extend_pairs(query(&search));
        let found: Found = answered(send(&request(url.as_str())?, None)?)?;
        Ok(SubtitleResults {
            results: found
                .data
                .into_iter()
                .filter_map(Subtitle::result)
                .collect(),
            ..SubtitleResults::default()
        })
    }

    fn download(download: SubtitleDownload) -> Result<SubtitleFile, Error> {
        let file_id: u64 = (download.id.parse()).map_err(|_not_a_number| {
            Error::msg(format!("{:?} isn't an OpenSubtitles file id", download.id))
        })?;
        let link = link(file_id)?;
        // The link is on a file host, not the API, so its statuses don't mean what `accepted`
        // reads them as, such as a 403 meaning a wrong API key.
        let fetched = send(&HttpRequest::new(&link.link), None)?;
        if !(200..300).contains(&fetched.status_code()) {
            return Err(Refused::Status(fetched.status_code()).into());
        }
        Ok(SubtitleFile {
            // OpenSubtitles converts to `srt` unless asked for another format.
            format: (link.file_name.rsplit_once('.'))
                .map_or("srt", |(_, extension)| extension)
                .to_owned(),
            data: fetched.into_body(),
            ..SubtitleFile::default()
        })
    }
}

nama_pdk::subtitle_source!(OpenSubtitles);

/// OpenSubtitles' search parameters for `search`: by one of its ids, or by its file name without
/// one. Sorted by name and lowercase, since OpenSubtitles redirects any other search to that one.
fn query(search: &SubtitleSearch) -> Vec<(&'static str, String)> {
    let is_episode = search.kind == TitleKind::TITLE_KIND_SHOW;
    let mut query = vec![
        (
            "type",
            if is_episode { "episode" } else { "movie" }.to_owned(),
        ),
        ("languages", languages(&search.languages)),
    ];
    // OpenSubtitles keys its titles by IMDb id, and asks for ids without a prefix or leading zeros.
    let id = |namespace: &str| {
        (search.ids.iter())
            .find(|id| id.namespace == namespace)
            .and_then(|id| id.value.trim_start_matches("tt").parse::<u64>().ok())
    };
    match (id("imdb"), id("tmdb"), is_episode) {
        (Some(imdb), _, false) => query.push(("imdb_id", imdb.to_string())),
        (None, Some(tmdb), false) => query.push(("tmdb_id", tmdb.to_string())),
        (Some(imdb), _, true) => query.push(("parent_imdb_id", imdb.to_string())),
        (None, Some(tmdb), true) => query.push(("parent_tmdb_id", tmdb.to_string())),
        (None, None, _) => query.push(("query", search.file_name.clone())),
    }
    if is_episode {
        query.extend(
            search
                .season
                .map(|season| ("season_number", season.to_string())),
        );
        query.extend(
            search
                .episode
                .map(|episode| ("episode_number", episode.to_string())),
        );
    }
    query.extend(search.hash.clone().map(|hash| ("moviehash", hash)));
    for (_, value) in &mut query {
        *value = value.to_lowercase();
    }
    query.sort_unstable();
    query
}

/// `tags` as OpenSubtitles' `languages` parameter: lowercase, sorted, and joined by commas.
fn languages(tags: &[String]) -> String {
    let mut tags: Vec<_> = tags.iter().map(|tag| tag.to_lowercase()).collect();
    tags.sort_unstable();
    tags.join(",")
}

/// A page of OpenSubtitles' search results.
#[derive(Deserialize)]
struct Found {
    data: Vec<Subtitle>,
}

/// One subtitle OpenSubtitles found.
#[derive(Deserialize)]
struct Subtitle {
    attributes: Attributes,
}

#[derive(Deserialize)]
struct Attributes {
    language: Option<String>,
    #[serde(default)]
    release: String,
    #[serde(default)]
    hearing_impaired: bool,
    files: Vec<File>,
}

/// One of a subtitle's files, one per CD.
#[derive(Deserialize)]
struct File {
    file_id: u64,
}

impl Subtitle {
    /// The result for this subtitle, or `None` for one in several files, which fits no single
    /// Version, or in no language.
    fn result(self) -> Option<SubtitleResult> {
        let Attributes {
            language,
            release,
            hearing_impaired,
            files,
        } = self.attributes;
        let [File { file_id }] = files.as_slice() else {
            return None;
        };
        Some(SubtitleResult {
            id: file_id.to_string(),
            language: language?,
            name: release,
            sdh: hearing_impaired,
            ..SubtitleResult::default()
        })
    }
}

/// Where a download's file is.
#[derive(Deserialize)]
struct Link {
    link: String,
    #[serde(default)]
    file_name: String,
}

/// Where the file `file_id` is, asked with the login the Store keeps, if it does and OpenSubtitles
/// still takes its token, else with a fresh one the Store then keeps. OpenSubtitles limits logins
/// tighter than other calls, and its 401 is the only sign a token expired.
fn link(file_id: u64) -> Result<Link, Error> {
    let username = setting(USERNAME)?;
    if let Some(kept) = kept_login().filter(|kept| kept.username == username) {
        match accepted(ask_link(&kept, file_id)?) {
            Err(Refused::Login) => {}
            answer => return parsed(&answer?),
        }
    }
    let login = login()?;
    keep(&login);
    answered(ask_link(&login, file_id)?)
}

/// The login the Store keeps, or `None` when it keeps none or isn't granted.
fn kept_login() -> Option<LoggedIn> {
    let kept = kernel::store_get(LOGIN_KEY, None).ok()??;
    json::from_slice(&kept).ok()
}

/// Keeps `login` in the Store for the next download. When the Admin didn't grant the Store, that
/// download logs in again (ADR 0048: the Plugin carries on).
fn keep(login: &LoggedIn) {
    if let Ok(login) = json::to_vec(login) {
        let _refused: Result<(), Error> = kernel::store_put(LOGIN_KEY, None, &login);
    }
}

/// OpenSubtitles' answer to asking with `login` where the file `file_id` is, asked of the host
/// its login answered: OpenSubtitles answers each account the host its later requests go to, such
/// as `vip-api.opensubtitles.com` for a VIP one.
fn ask_link(login: &LoggedIn, file_id: u64) -> Result<Response, Error> {
    let LoggedIn {
        token, base_url, ..
    } = login;
    let request = post(&format!("https://{base_url}/api/v1/download"))?
        .with_header("Authorization", format!("Bearer {token}"));
    send(&request, Some(json!({ "file_id": file_id })))
}

/// What a login answers: the token a download needs, and the API host to send it to, kept with
/// the username it logged in, so a login for an account the Admin replaced isn't used.
#[derive(Deserialize, Serialize)]
struct LoggedIn {
    token: String,
    base_url: String,
    // OpenSubtitles' answer leaves it out.
    #[serde(default)]
    username: String,
}

/// Logs in to the settings' account.
fn login() -> Result<LoggedIn, Error> {
    let username = setting(USERNAME)?;
    let body = json!({ "username": username, "password": setting(PASSWORD)? });
    let logged_in: LoggedIn = answered(send(&post(&format!("{API_URL}login"))?, Some(body))?)?;
    Ok(LoggedIn {
        username,
        ..logged_in
    })
}

/// A request to OpenSubtitles' API at `url`, with the headers it wants on every one.
fn request(url: &str) -> Result<HttpRequest, Error> {
    Ok(HttpRequest::new(url)
        .with_header("Api-Key", setting(API_KEY)?)
        // OpenSubtitles asks every app to name itself and its version.
        .with_header("User-Agent", concat!("Nama v", env!("CARGO_PKG_VERSION")))
        .with_header("Accept", "application/json"))
}

/// A POST of JSON to OpenSubtitles' API at `url`.
fn post(url: &str) -> Result<HttpRequest, Error> {
    Ok(request(url)?
        .with_method("POST")
        .with_header("Content-Type", "application/json"))
}

fn send(request: &HttpRequest, body: Option<json::Value>) -> Result<Response, Error> {
    nama_pdk::http_request(request, body.map(Json), "OpenSubtitles didn't answer")
}

/// What OpenSubtitles answered with `response`, read as JSON.
///
/// # Errors
/// If OpenSubtitles refused the request, or answered with something unexpected.
fn answered<T: DeserializeOwned>(response: Response) -> Result<T, Error> {
    parsed(&accepted(response)?)
}

/// `response` read as JSON.
///
/// # Errors
/// If OpenSubtitles answered with something unexpected.
fn parsed<T: DeserializeOwned>(response: &Response) -> Result<T, Error> {
    response.json().map_err(|err| {
        Error::msg(format!(
            "OpenSubtitles answered with something unexpected: {err}"
        ))
    })
}

/// `response` if OpenSubtitles took the request, else why it refused.
fn accepted(response: Response) -> Result<Response, Refused> {
    #[derive(Deserialize)]
    struct Quota {
        reset_time: Option<String>,
    }
    Err(match response.status_code() {
        200..300 => return Ok(response),
        401 => Refused::Login,
        403 => Refused::ApiKey,
        406 => Refused::Quota {
            renews_in: (response.json::<Quota>().ok()).and_then(|quota| quota.reset_time),
        },
        429 => Refused::RateLimit {
            // OpenSubtitles gives seconds, though HTTP allows a date too.
            retry_after: response
                .header("Retry-After")
                .and_then(|after| after.parse().ok()),
        },
        status => Refused::Status(status),
    })
}

/// Why OpenSubtitles refused a request, which the Plugins page shows as the call's failure.
#[derive(Debug, PartialEq, Eq)]
enum Refused {
    /// The API key is wrong.
    ApiKey,
    /// The username or password is wrong, or a kept login's token expired.
    Login,
    /// The Server sent more requests than OpenSubtitles takes, until the seconds it gives pass.
    RateLimit { retry_after: Option<u64> },
    /// The account has downloaded all it may today, until it renews in the time OpenSubtitles
    /// gives, such as `3 hours and 12 minutes`.
    Quota { renews_in: Option<String> },
    /// Any other HTTP status.
    Status(u16),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey => write!(f, "OpenSubtitles refused the API key"),
            Self::Login => write!(f, "OpenSubtitles refused the username or password"),
            Self::RateLimit { retry_after } => {
                write!(f, "OpenSubtitles is limiting this Server's requests; ")?;
                match retry_after {
                    Some(seconds) => write!(f, "try again in {seconds} s"),
                    None => write!(f, "try again in a few seconds"),
                }
            }
            Self::Quota { renews_in } => {
                write!(
                    f,
                    "the OpenSubtitles account has used its daily download quota"
                )?;
                match renews_in {
                    Some(renews_in) => write!(f, ", which renews in {renews_in}"),
                    None => Ok(()),
                }
            }
            Self::Status(status) => write!(
                f,
                "OpenSubtitles refused the request with HTTP status {status}"
            ),
        }
    }
}

impl std::error::Error for Refused {}

/// The setting `key`, which the Server sets before every call.
fn setting(key: &str) -> Result<String, Error> {
    nama_pdk::setting(key)?.ok_or_else(|| Error::msg(format!("the setting {key} isn't set")))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
mod fake_opensubtitles;
