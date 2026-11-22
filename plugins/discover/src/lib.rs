//! The Discover Installed Plugin: a Home row source of rows every User sees, such as Trending this
//! week, from TMDB, the Admin picking which it offers. The Server builds it for wasm32 and bundles
//! it in its binary.

use std::time::Duration;

use nama_pdk::{
    CheckedSettings, HomeRow, HomeRowSource, HomeRowTitle, HomeRows, Manifest, OutsideService,
    Permission, SettingField, SettingKind, TitleId, TitleKind, checked_settings,
    choices_setting_field,
    extism_pdk::{Error, HttpRequest, json},
    nama_plugin_proto::nama::plugin::v1::Texts,
    services, setting_default, setting_field, setting_value,
};
use serde::Deserialize;
use url::Url;

const API_KEY: &str = "api_key";
const REGION: &str = "region";
const SHARED_ROWS: &str = "shared_rows";

/// The most rows the Kernel keeps from one answer (ADR 0091): picking more would lose some.
const MOST_PICKS: usize = 20;

const TMDB: OutsideService = OutsideService::new("TMDB").credential("API key");

/// Where TMDB's v3 API is.
const API_URL: &str = "https://api.themoviedb.org/3/";

const TRENDING: &str = "trending-week";

/// Every row Discover offers every User, by key and heading, in the order a form lists them and
/// home shows them. A key is also the row's, so it never changes.
const SHARED: [(&str, &str); 18] = [
    (TRENDING, "Trending this week"),
    ("popular-movies", "Popular movies"),
    ("popular-shows", "Popular shows"),
    ("top-rated-movies", "Top rated movies"),
    ("top-rated-shows", "Top rated shows"),
    ("now-in-theaters", "Now in theaters"),
    ("coming-soon", "Coming soon"),
    ("on-the-air", "On the air this week"),
    ("critically-acclaimed", "Critically acclaimed"),
    ("80s", "80s favourites"),
    ("90s", "90s favourites"),
    ("2000s", "2000s favourites"),
    ("under-90-minutes", "Under 90 minutes"),
    ("hidden-gems", "Hidden gems"),
    ("best-picture", "Best Picture winners"),
    ("oscar-nominees", "Oscar nominees"),
    ("palme-dor", "Palme d'Or winners"),
    ("golden-globe-dramas", "Golden Globe best dramas"),
];

/// How long a shared row stays fresh: TMDB's charts move daily at most.
const FRESH_FOR: Duration = Duration::from_hours(24);

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

/// What `check_settings()` returns: TMDB's countries, as Region's choices, asked with the API key
/// so a key TMDB refuses fails the check.
///
/// # Errors
/// If more than [`MOST_PICKS`] shared rows are picked, or TMDB refuses the key or can't answer.
fn check_settings() -> Result<CheckedSettings, Error> {
    let picked = nama_pdk::setting_picked(SHARED_ROWS)?.len();
    if picked > MOST_PICKS {
        return Err(Error::msg(format!(
            "pick at most {MOST_PICKS} shared rows, not {picked}"
        )));
    }
    let mut countries: Vec<Country> =
        Api::from_settings()?.get(&["configuration", "countries"], &[])?;
    countries.sort_by(|a, b| a.english_name.cmp(&b.english_name));
    Ok(checked_settings([(
        REGION,
        (countries.into_iter()).map(|country| (country.iso_3166_1, country.english_name)),
    )]))
}

/// A country as TMDB lists it.
#[derive(Deserialize)]
struct Country {
    iso_3166_1: String,
    english_name: String,
}

const SETUP: &str = "\
1. Sign in to [TMDB](https://www.themoviedb.org/), or sign up there for free.
2. Under [Settings, API](https://www.themoviedb.org/settings/api), ask for an API key, then copy its API Key, not its Read Access Token, into the TMDB API key setting below.
3. Press Test to check the key and list the countries Region offers.
4. Pick up to 20 Shared rows. Users hide or reorder them in their Home layout.
";

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let api_key = SettingField {
        required: true,
        ..setting_field(
            API_KEY,
            "TMDB API key",
            "Your TMDB API key, from the API section of your TMDB account's settings.",
            SettingKind::SETTING_KIND_SECRET,
        )
    };
    let region = SettingField {
        default_value: setting_default(setting_value::Value::TextValue("US".to_owned())),
        ..setting_field(
            REGION,
            "Region",
            "The country whose cinemas Now in theaters and Coming soon follow. Test lists the \
             countries.",
            SettingKind::SETTING_KIND_CHOICE,
        )
    };
    let defaults = [TRENDING, "top-rated-movies", "top-rated-shows"].map(str::to_owned);
    let shared_rows = SettingField {
        default_value: setting_default(
            Texts {
                values: defaults.to_vec(),
                ..Texts::default()
            }
            .into(),
        ),
        ..choices_setting_field(
            SHARED_ROWS,
            "Shared rows",
            format!("The rows every User sees, at most {MOST_PICKS}."),
            SHARED,
        )
    };
    Manifest {
        name: "Discover".to_owned(),
        description: "Shows rows such as Trending this week on the home page. This product uses \
                      the TMDB API but is not endorsed or certified by TMDB."
            .to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::HOME_ROW)],
        allowed_hosts: vec!["api.themoviedb.org".to_owned()],
        // Asked from the start, so an Admin grants once what Discover's rows for each User and its
        // award rows, kept in the Store, will need.
        permissions: vec![
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_WATCHLISTS.into(),
            Permission::PERMISSION_STORE.into(),
        ],
        settings: vec![api_key, region, shared_rows],
        setup_markdown: SETUP.to_owned(),
        homepage_url: "https://www.themoviedb.org/".to_owned(),
        source_url: concat!(env!("CARGO_PKG_REPOSITORY"), "/tree/main/plugins/discover").to_owned(),
        license: env!("CARGO_PKG_LICENSE").to_owned(),
        ..nama_pdk::manifest("discover", env!("CARGO_PKG_VERSION"))
    }
}

/// Serves the rows the Admin picks.
struct Discover;

impl HomeRowSource for Discover {
    /// A row for each shared row picked, in [`SHARED`]'s order.
    ///
    /// # Errors
    /// If the API key isn't set, or TMDB can't answer for any row: a refresh shows every row or
    /// none, so the Kernel keeps the rows it had.
    fn shared_rows() -> Result<HomeRows, Error> {
        let api = Api::from_settings()?;
        let picked = nama_pdk::setting_picked(SHARED_ROWS)?;
        let mut rows = Vec::new();
        for (key, heading) in SHARED
            .into_iter()
            .filter(|(key, _)| picked.iter().any(|pick| pick == key))
        {
            // ponytail: a picked row Discover doesn't answer yet is left out until it's built.
            let Some(titles) = shared_titles(&api, key)? else {
                continue;
            };
            rows.push(HomeRow {
                key: key.to_owned(),
                heading: heading.to_owned(),
                ttl_seconds: FRESH_FOR.as_secs().try_into()?,
                titles,
                ..HomeRow::default()
            });
        }
        Ok(HomeRows {
            rows,
            ..HomeRows::default()
        })
    }
}

nama_pdk::home_row_source!(Discover, [shared_rows]);

/// The titles of the shared row `key` from TMDB, or `None` for a row Discover doesn't answer yet.
///
/// # Errors
/// If TMDB can't answer, or the Region isn't set when the row needs it.
fn shared_titles(api: &Api, key: &str) -> Result<Option<Vec<HomeRowTitle>>, Error> {
    const MOVIES: Option<TitleKind> = Some(TitleKind::TITLE_KIND_MOVIE);
    const SHOWS: Option<TitleKind> = Some(TitleKind::TITLE_KIND_SHOW);
    let region = || nama_pdk::required_setting(REGION);
    let titles = match key {
        TRENDING => api.titles(&["trending", "all", "week"], &[], None)?,
        "popular-movies" => api.titles(&["movie", "popular"], &[], MOVIES)?,
        "popular-shows" => api.titles(&["tv", "popular"], &[], SHOWS)?,
        "top-rated-movies" => api.titles(&["movie", "top_rated"], &[], MOVIES)?,
        "top-rated-shows" => api.titles(&["tv", "top_rated"], &[], SHOWS)?,
        "now-in-theaters" => {
            api.titles(&["movie", "now_playing"], &[("region", &region()?)], MOVIES)?
        }
        "coming-soon" => api.titles(&["movie", "upcoming"], &[("region", &region()?)], MOVIES)?,
        "on-the-air" => api.titles(&["tv", "on_the_air"], &[], SHOWS)?,
        // Popular first, so it differs from Top rated movies, which sorts by rating.
        "critically-acclaimed" => api.discover_movies(&[
            ("sort_by", "popularity.desc"),
            ("vote_average.gte", "8"),
            ("vote_count.gte", "5000"),
        ])?,
        "80s" => api.discover_movies(&favourites_of("1980-01-01", "1989-12-31"))?,
        "90s" => api.discover_movies(&favourites_of("1990-01-01", "1999-12-31"))?,
        "2000s" => api.discover_movies(&favourites_of("2000-01-01", "2009-12-31"))?,
        // At least an hour, leaving out short films.
        "under-90-minutes" => api.discover_movies(&[
            ("with_runtime.gte", "60"),
            ("with_runtime.lte", "89"),
            ("sort_by", "vote_average.desc"),
            ("vote_count.gte", "1000"),
        ])?,
        // The vote floor keeps out titles a handful of fans rated 10.
        "hidden-gems" => api.discover_movies(&[
            ("vote_average.gte", "7.5"),
            ("vote_count.gte", "200"),
            ("vote_count.lte", "1000"),
            ("sort_by", "vote_average.desc"),
        ])?,
        _ => return Ok(None),
    };
    Ok(Some(titles))
}

/// TMDB's search for the best-rated Movies released from `first` to `last`, both `YYYY-MM-DD`,
/// the vote floor keeping out titles few rated.
fn favourites_of<'a>(first: &'a str, last: &'a str) -> [(&'static str, &'a str); 4] {
    [
        ("primary_release_date.gte", first),
        ("primary_release_date.lte", last),
        ("sort_by", "vote_average.desc"),
        ("vote_count.gte", "1000"),
    ]
}

/// TMDB's API, asked with the Admin's key.
struct Api {
    key: String,
}

/// A page of TMDB's titles.
#[derive(Deserialize)]
struct Page {
    results: Vec<Listing>,
}

/// One title in a page, or a person in a trending one.
#[derive(Deserialize)]
struct Listing {
    id: u64,
    /// Only in a page mixing kinds, such as trending.
    #[serde(default)]
    media_type: String,
}

impl Api {
    fn from_settings() -> Result<Self, Error> {
        Ok(Self {
            key: nama_pdk::required_setting(API_KEY)?.trim().to_owned(),
        })
    }

    /// The Movies and Shows in the first page at `path` asked with `query`, leaving out people.
    /// `kind` is every title's when TMDB lists one kind there, or `None` where each title says its
    /// own, as in trending.
    // ponytail: one page, TMDB's 20 titles; ask for more pages if a row of 20 feels short.
    fn titles(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        kind: Option<TitleKind>,
    ) -> Result<Vec<HomeRowTitle>, Error> {
        let page: Page = self.get(path, query)?;
        Ok((page.results.iter())
            .filter_map(|listing| title(listing, kind))
            .collect())
    }

    /// The Movies TMDB's search finds for `query`.
    fn discover_movies(&self, query: &[(&str, &str)]) -> Result<Vec<HomeRowTitle>, Error> {
        self.titles(
            &["discover", "movie"],
            query,
            Some(TitleKind::TITLE_KIND_MOVIE),
        )
    }

    /// TMDB's answer at `path` under [`API_URL`], asked with `query`.
    ///
    /// # Errors
    /// If TMDB didn't answer, refused, or answered with something unexpected.
    fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
    ) -> Result<T, Error> {
        let mut url = Url::parse(API_URL)?;
        url.path_segments_mut()
            .map_err(|()| Error::msg("TMDB's API URL has no path"))?
            .pop_if_empty()
            .extend(path);
        url.query_pairs_mut()
            .extend_pairs(query)
            .append_pair("api_key", &self.key);
        let response = TMDB.send(&HttpRequest::new(url.as_str()), None::<String>)?;
        json::from_slice(TMDB.accepted(response)?.body())
            .map_err(|err| Error::msg(format!("TMDB answered with something unexpected: {err}")))
    }
}

/// The Home row title of `listing`, of `kind` or else its own, or `None` for a person.
fn title(listing: &Listing, kind: Option<TitleKind>) -> Option<HomeRowTitle> {
    let kind = match (kind, listing.media_type.as_str()) {
        (Some(kind), _) => kind,
        (None, "movie") => TitleKind::TITLE_KIND_MOVIE,
        (None, "tv") => TitleKind::TITLE_KIND_SHOW,
        (None, _) => return None,
    };
    Some(HomeRowTitle {
        kind: kind.into(),
        id: TitleId {
            namespace: "tmdb".to_owned(),
            value: listing.id.to_string(),
            ..TitleId::default()
        }
        .into(),
        ..HomeRowTitle::default()
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
