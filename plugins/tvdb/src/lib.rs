//! The TVDB Installed Plugin: a title source serving Shows from TVDB's v4 API, with ids in the
//! `tvdb` namespace and the IMDb and TMDB ids TVDB reports. Movies are left to TMDB. The Server
//! builds it for wasm32 and bundles it in its binary.

use nama_pdk::{
    Candidate, Episode, Image, ImageAnswer, ImageAsked, Manifest, OutsideService, SearchQuery,
    SearchResults, Season, SeasonAnswer, SeasonAsked, SettingField, SettingKind, Show, ShowAnswer,
    TitleAsked, TitleId, TitleKind, TitleSource,
    extism_pdk::{Error, HttpRequest, Json, json},
    required_setting, services, setting_default, setting_field, setting_value,
};
use serde::{Deserialize, de::DeserializeOwned};

/// The namespace TVDB's ids are kept under.
const NAMESPACE: &str = "tvdb";
const API_KEY: &str = "api_key";
const PIN: &str = "pin";
const API_URL: &str = "api_url";
const ARTWORK_URL: &str = "artwork_url";

const TVDB: OutsideService = OutsideService::new("TVDB").credential("API key or subscriber PIN");

/// The host of TVDB's images, which is sent no key.
const ARTWORK_HOST: OutsideService = OutsideService::new("TVDB's artwork host");

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, CheckedSettings, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    /// Logs in, so testing the settings reports a key TVDB refuses.
    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        super::Api::login()?;
        Ok(Buffa(CheckedSettings::default()))
    }
}

const SETUP: &str = "\
1. Sign in to [TVDB](https://thetvdb.com/), or sign up there for free.
2. Get an API key from your [API keys page](https://thetvdb.com/dashboard/account/apikey) and paste it into the API key setting below.
3. If TVDB says your key needs a subscriber PIN, enter it in the Subscriber PIN setting.
";

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let url = |key: &str, label: &str, description: &str, default: &str| SettingField {
        default_value: setting_default(setting_value::Value::TextValue(default.to_owned())),
        ..setting_field(key, label, description, SettingKind::SETTING_KIND_URL)
    };
    Manifest {
        name: "TVDB".to_owned(),
        description: "Finds Shows and their Seasons and Episodes on TVDB.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::TITLE_SOURCE)],
        title_namespace: NAMESPACE.to_owned(),
        allowed_hosts: vec![
            "api4.thetvdb.com".to_owned(),
            "artworks.thetvdb.com".to_owned(),
        ],
        settings: vec![
            SettingField {
                required: true,
                ..setting_field(
                    API_KEY,
                    "API key",
                    "Your TVDB API key, from your account's API keys page on thetvdb.com.",
                    SettingKind::SETTING_KIND_SECRET,
                )
            },
            setting_field(
                PIN,
                "Subscriber PIN",
                "Your TVDB subscriber PIN, if your API key needs one.",
                SettingKind::SETTING_KIND_SECRET,
            ),
            url(
                API_URL,
                "API URL",
                "Where TVDB's v4 API is. Change it only for a server of your own.",
                "https://api4.thetvdb.com/v4",
            ),
            url(
                ARTWORK_URL,
                "Artwork URL",
                "Where TVDB's images are. Change it only for a server of your own.",
                "https://artworks.thetvdb.com",
            ),
        ],
        setup_markdown: SETUP.to_owned(),
        homepage_url: "https://thetvdb.com/".to_owned(),
        source_url: concat!(env!("CARGO_PKG_REPOSITORY"), "/tree/main/plugins/tvdb").to_owned(),
        license: env!("CARGO_PKG_LICENSE").to_owned(),
        ..nama_pdk::manifest("tvdb", env!("CARGO_PKG_VERSION"))
    }
}

/// Serves Shows from TVDB.
struct Tvdb;

impl TitleSource for Tvdb {
    fn search(query: SearchQuery) -> Result<SearchResults, Error> {
        if query.kind != TitleKind::TITLE_KIND_SHOW {
            return Ok(SearchResults::default());
        }
        let year = query.year.map(|year| format!("&year={year}"));
        let path = format!(
            "search?query={}&type=series{}",
            encoded(&query.title),
            year.unwrap_or_default()
        );
        let found: Vec<Found> = Api::login()?.get(&path)?.unwrap_or_default();
        Ok(SearchResults {
            candidates: found.into_iter().filter_map(Found::candidate).collect(),
            ..SearchResults::default()
        })
    }

    fn show(asked: TitleAsked) -> Result<ShowAnswer, Error> {
        let id = match tvdb_id(asked.id.as_option())? {
            Lookup::Found(id) => id,
            Lookup::Missing => return Ok(ShowAnswer::default()),
            Lookup::Unsupported => {
                return Ok(ShowAnswer {
                    unsupported_id: true,
                    ..ShowAnswer::default()
                });
            }
        };
        let path = format!("series/{id}/extended?meta=translations");
        let series: Option<Series> = Api::login()?.get(&path)?;
        Ok(ShowAnswer {
            show: series.map(Series::show).into(),
            ..ShowAnswer::default()
        })
    }

    fn season(asked: SeasonAsked) -> Result<SeasonAnswer, Error> {
        let id = match tvdb_id(asked.show.as_option())? {
            Lookup::Found(id) => id,
            Lookup::Missing => return Ok(SeasonAnswer::default()),
            Lookup::Unsupported => {
                return Ok(SeasonAnswer {
                    unsupported_id: true,
                    ..SeasonAnswer::default()
                });
            }
        };
        let path = format!("series/{id}/episodes/default?season={}", asked.season);
        let episodes: Option<Episodes> = Api::login()?.get(&path)?;
        Ok(SeasonAnswer {
            season: episodes
                .and_then(|episodes| season(asked.season, episodes.episodes))
                .into(),
            ..SeasonAnswer::default()
        })
    }

    fn image(asked: ImageAsked) -> Result<ImageAnswer, Error> {
        // A path that isn't one keeps the request on the artwork host.
        if !asked.path.starts_with('/') {
            return Ok(ImageAnswer::default());
        }
        let url = format!(
            "{}{}",
            required_setting(ARTWORK_URL)?.trim_end_matches('/'),
            asked.path
        );
        // Sent as the service, whose "didn't answer" Admins already read; refused as the host.
        let response = TVDB.send(&HttpRequest::new(url), None::<String>)?;
        let Some(response) = ARTWORK_HOST.found(response)? else {
            return Ok(ImageAnswer::default());
        };
        Ok(ImageAnswer {
            image: Image {
                // The Server gives a Plugin no response headers, so the type comes from the path.
                content_type: content_type(&asked.path).to_owned(),
                bytes: response.into_body(),
                ..Image::default()
            }
            .into(),
            ..ImageAnswer::default()
        })
    }
}

nama_pdk::title_source!(Tvdb, [search, show, season, image]);

/// TVDB's API, logged in.
struct Api {
    url: String,
    token: String,
}

impl Api {
    /// Logs in with the settings' API key and PIN.
    ///
    /// ponytail: one login per call, since each call is a fresh instance; cache the token when
    /// TVDB's rate limits bite.
    fn login() -> Result<Self, Error> {
        let url = required_setting(API_URL)?.trim_end_matches('/').to_owned();
        let api_key = required_setting(API_KEY)?;
        let body = match nama_pdk::setting(PIN)? {
            Some(pin) => json::json!({ "apikey": api_key, "pin": pin.trim() }),
            None => json::json!({ "apikey": api_key }),
        };
        let request = HttpRequest::new(format!("{url}/login"))
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        let Some(response) = TVDB.found(TVDB.send(&request, Some(Json(body)))?)? else {
            return Err(Error::msg(
                "TVDB has no login at the API URL (HTTP status 404)",
            ));
        };
        let Login { token } = data(response.body())?;
        Ok(Self { url, token })
    }

    /// The `data` TVDB answers `GET {path}` with, or `None` for a 404.
    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, Error> {
        let request = HttpRequest::new(format!("{}/{path}", self.url))
            .with_header("Authorization", format!("Bearer {}", self.token));
        let response = TVDB.send(&request, None::<String>)?;
        let Some(response) = TVDB.found(response)? else {
            return Ok(None);
        };
        data(response.body()).map(Some)
    }
}

/// The `data` of a TVDB answer.
fn data<T: DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    #[derive(Deserialize)]
    struct Answer<T> {
        data: T,
    }
    let answer: Answer<T> = json::from_slice(body)
        .map_err(|err| Error::msg(format!("TVDB answered with something unexpected: {err}")))?;
    Ok(answer.data)
}

/// What an asked id comes to at TVDB.
#[derive(Debug, PartialEq, Eq)]
enum Lookup {
    /// The TVDB id of the title.
    Found(u64),
    /// Not a number TVDB could have.
    Missing,
    /// In a namespace TVDB can't look titles up by.
    Unsupported,
}

/// What the TVDB id `id` names comes to.
///
/// # Errors
/// If there is no id.
fn tvdb_id(id: Option<&TitleId>) -> Result<Lookup, Error> {
    let id = id.ok_or_else(|| Error::msg("TVDB was asked for no id"))?;
    if id.namespace != NAMESPACE {
        return Ok(Lookup::Unsupported);
    }
    Ok(id.value.parse().map_or(Lookup::Missing, Lookup::Found))
}

fn title_id(namespace: &str, value: impl Into<String>) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.into(),
        ..TitleId::default()
    }
}

/// `text` percent-encoded for a query string.
fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            byte => format!("%{byte:02X}"),
        })
        .collect()
}

/// The path of the TVDB artwork at `url`, such as `/banners/v4/series/1/posters/a.jpg`, which the
/// `image` export fetches from the artwork URL setting.
fn artwork_path(url: Option<String>) -> Option<String> {
    let url = url.filter(|url| !url.is_empty())?;
    match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/').map(|(_, path)| format!("/{path}")),
        None => url.starts_with('/').then_some(url),
    }
}

fn content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

/// The ISO 3166-1 alpha-2 code for TVDB's alpha-3 country `code`, for the countries Shows most
/// often come from.
///
/// ponytail: a short table, so a rarer country is left out; take the full ISO list if a Library
/// needs one.
fn country(code: &str) -> Option<&'static str> {
    const COUNTRIES: &[(&str, &str)] = &[
        ("usa", "US"),
        ("gbr", "GB"),
        ("can", "CA"),
        ("aus", "AU"),
        ("nzl", "NZ"),
        ("irl", "IE"),
        ("deu", "DE"),
        ("fra", "FR"),
        ("esp", "ES"),
        ("ita", "IT"),
        ("nld", "NL"),
        ("bel", "BE"),
        ("swe", "SE"),
        ("nor", "NO"),
        ("dnk", "DK"),
        ("fin", "FI"),
        ("isl", "IS"),
        ("pol", "PL"),
        ("rus", "RU"),
        ("tur", "TR"),
        ("isr", "IL"),
        ("ind", "IN"),
        ("chn", "CN"),
        ("twn", "TW"),
        ("hkg", "HK"),
        ("jpn", "JP"),
        ("kor", "KR"),
        ("tha", "TH"),
        ("mex", "MX"),
        ("bra", "BR"),
        ("arg", "AR"),
        ("col", "CO"),
        ("zaf", "ZA"),
    ];
    let code = code.to_ascii_lowercase();
    COUNTRIES
        .iter()
        .find(|(alpha3, _)| *alpha3 == code)
        .map(|&(_, alpha2)| alpha2)
}

/// An id TVDB has for a title elsewhere.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteId {
    id: String,
    source_name: String,
}

/// `tvdb_id` and the IMDb and TMDB ids among `remote`, the ids the Server knows namespaces for.
fn ids(tvdb_id: String, remote_ids: Vec<RemoteId>) -> Vec<TitleId> {
    let mut ids = vec![title_id(NAMESPACE, tvdb_id)];
    for remote in remote_ids {
        let namespace = match remote.source_name.as_str() {
            "IMDB" => "imdb",
            name if name.starts_with("TheMovieDB") => "tmdb",
            _ => continue,
        };
        if !remote.id.is_empty() && !ids.iter().any(|id| id.namespace == namespace) {
            ids.push(title_id(namespace, remote.id));
        }
    }
    ids
}

#[derive(Deserialize)]
struct Login {
    token: String,
}

/// A Show a search found.
#[derive(Deserialize)]
struct Found {
    tvdb_id: String,
    name: String,
    year: Option<String>,
    country: Option<String>,
    image_url: Option<String>,
    #[serde(default)]
    translations: std::collections::HashMap<String, String>,
    #[serde(default)]
    remote_ids: Vec<RemoteId>,
}

impl Found {
    fn candidate(mut self) -> Option<Candidate> {
        if self.tvdb_id.is_empty() {
            return None;
        }
        let title = self
            .translations
            .remove("eng")
            .unwrap_or_else(|| self.name.clone());
        Some(Candidate {
            id: title_id(NAMESPACE, self.tvdb_id.clone()).into(),
            ids: ids(self.tvdb_id, self.remote_ids),
            original_title: (self.name != title).then_some(self.name),
            title,
            year: self.year.and_then(|year| year.parse().ok()),
            poster: artwork_path(self.image_url),
            origin_country: self
                .country
                .as_deref()
                .and_then(country)
                .map(str::to_owned)
                .into_iter()
                .collect(),
            ..Candidate::default()
        })
    }
}

/// A Show's extended record, with its translations.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Series {
    id: u64,
    name: String,
    overview: Option<String>,
    first_aired: Option<String>,
    image: Option<String>,
    #[serde(default)]
    remote_ids: Vec<RemoteId>,
    #[serde(default)]
    artworks: Vec<Artwork>,
    #[serde(default)]
    seasons: Vec<SeriesSeason>,
    translations: Option<Translations>,
}

#[derive(Deserialize)]
struct Artwork {
    image: String,
    #[serde(rename = "type")]
    kind: u32,
    #[serde(default)]
    score: f64,
}

/// TVDB's artwork type for a Show's background.
const BACKGROUND: u32 = 3;

#[derive(Deserialize)]
struct SeriesSeason {
    number: u32,
    #[serde(rename = "type")]
    order: SeasonOrder,
}

#[derive(Deserialize)]
struct SeasonOrder {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Translations {
    #[serde(default)]
    name_translations: Vec<Translation>,
    #[serde(default)]
    overview_translations: Vec<Translation>,
}

#[derive(Deserialize)]
struct Translation {
    language: String,
    name: Option<String>,
    overview: Option<String>,
}

impl Series {
    fn show(self) -> Show {
        let english = |translations: &[Translation], text: fn(&Translation) -> &Option<String>| {
            translations
                .iter()
                .find(|translation| translation.language == "eng")
                .and_then(|translation| text(translation).clone())
        };
        let translations = self.translations.as_ref();
        let name = translations.and_then(|t| english(&t.name_translations, |t| &t.name));
        let overview = translations
            .and_then(|t| english(&t.overview_translations, |t| &t.overview))
            .or(self.overview);
        let title = name.unwrap_or_else(|| self.name.clone());
        let backdrop = self
            .artworks
            .into_iter()
            .filter(|artwork| artwork.kind == BACKGROUND)
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .map(|artwork| artwork.image);
        // A Show's Seasons in other orders, such as DVD, share their numbers with the aired ones.
        let mut seasons: Vec<u32> = self
            .seasons
            .iter()
            .filter(|season| season.order.kind == "official")
            .map(|season| season.number)
            .collect();
        seasons.sort_unstable();
        seasons.dedup();
        Show {
            ids: ids(self.id.to_string(), self.remote_ids),
            original_title: (self.name != title).then_some(self.name),
            title,
            overview: overview.filter(|overview| !overview.is_empty()),
            first_air_date: self.first_aired.filter(|date| !date.is_empty()),
            poster: artwork_path(self.image),
            backdrop: artwork_path(backdrop),
            seasons,
            ..Show::default()
        }
    }
}

#[derive(Deserialize)]
struct Episodes {
    #[serde(default)]
    episodes: Vec<SeasonEpisode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeasonEpisode {
    id: u64,
    name: Option<String>,
    aired: Option<String>,
    runtime: Option<u32>,
    overview: Option<String>,
    image: Option<String>,
    season_number: u32,
    number: u32,
}

/// Season `number` made of `episodes`, `None` if it has none.
fn season(number: u32, episodes: Vec<SeasonEpisode>) -> Option<Season> {
    let episodes: Vec<Episode> = episodes
        .into_iter()
        .filter(|episode| episode.season_number == number)
        .map(|episode| Episode {
            ids: vec![title_id(NAMESPACE, episode.id.to_string())],
            season: number,
            number: episode.number,
            title: episode.name.unwrap_or_default(),
            overview: episode.overview.filter(|overview| !overview.is_empty()),
            air_date: episode.aired.filter(|date| !date.is_empty()),
            runtime_minutes: episode.runtime,
            still: artwork_path(episode.image),
            ..Episode::default()
        })
        .collect();
    if episodes.is_empty() {
        return None;
    }
    Some(Season {
        number,
        title: if number == 0 {
            "Specials".to_owned()
        } else {
            format!("Season {number}")
        },
        air_date: episodes.iter().filter_map(|e| e.air_date.clone()).min(),
        episodes,
        ..Season::default()
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
