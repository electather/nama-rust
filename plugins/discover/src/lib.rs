//! The Discover Installed Plugin: a Home row source of rows every User sees, such as Trending this
//! week from TMDB and Best Picture winners from Wikidata, and of each User's own, such as their
//! Watchlist, the Admin picking which it offers. The Server builds it for wasm32 and bundles it in
//! its binary.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    time::{Duration, UNIX_EPOCH},
};

use nama_pdk::{
    CheckedSettings, HomeRow, HomeRowSource, HomeRowTitle, HomeRows, Manifest, OutsideService,
    Permission, SettingField, SettingKind, Title, TitleId, TitleKind, TitleRefKind, UserRowsAsked,
    WatchState, WatchlistTitle, checked_settings, choices_setting_field,
    extism_pdk::{Error, HttpRequest, json},
    kernel,
    nama_plugin_proto::nama::plugin::v1::Texts,
    services, setting_default, setting_field, setting_value,
};
use serde::{Deserialize, Serialize};
use url::Url;

const API_KEY: &str = "api_key";
const REGION: &str = "region";
const SHARED_ROWS: &str = "shared_rows";
const USER_ROWS: &str = "user_rows";

/// The most rows the Kernel keeps from one answer (ADR 0091): picking more would lose some.
const MOST_PICKS: usize = 20;

/// The most characters of a heading the Kernel keeps (ADR 0058).
const MOST_HEADING_CHARS: usize = 40;

const TMDB: OutsideService = OutsideService::new("TMDB").credential("API key");

/// Where TMDB's v3 API is.
const API_URL: &str = "https://api.themoviedb.org/3/";

const WIKIDATA: OutsideService = OutsideService::new("Wikidata");

/// Where Wikidata's query service answers SPARQL.
const SPARQL_URL: &str = "https://query.wikidata.org/sparql";

/// Wikimedia's User-Agent policy asks a client to name itself and where to find who runs it.
const USER_AGENT: &str = concat!(
    "Nama-Discover/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

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

/// How long the Store keeps an award row's Movies before Wikidata is asked again: awards come
/// yearly, and Wikidata asks to be queried sparingly.
const AWARDS_FRESH_FOR: Duration = Duration::from_hours(30 * 24);

/// How long a shared row stays fresh: TMDB's charts move daily at most.
const FRESH_FOR: Duration = Duration::from_hours(24);

const BECAUSE: &str = "because";
const MORE_GENRE: &str = "more-genre";
const WATCH_AGAIN: &str = "watch-again";
const WATCHLIST: &str = "watchlist";

/// Every row Discover offers each User, by key and heading, as [`SHARED`] lists its rows. Because
/// you watched and More genre each stand for several rows, headed by what they follow.
const PER_USER: [(&str, &str); 6] = [
    (BECAUSE, "Because you watched"),
    (MORE_GENRE, "More genre"),
    ("starring", "Starring"),
    ("from", "From"),
    (WATCH_AGAIN, "Watch it again"),
    (WATCHLIST, "Your Watchlist"),
];

/// How long a User's row stays fresh. The Kernel asks again sooner once they play something.
const USER_FRESH_FOR: Duration = Duration::from_hours(6);

/// How many Because you watched rows a User gets, one for each title they played most recently.
const BECAUSE_ROWS: usize = 3;

/// How many More genre rows a User gets, one for each genre they played most.
const GENRE_ROWS: usize = 2;

/// The fewest votes a title needs on TMDB to be among a genre's top titles, so one a handful
/// rated highly doesn't top the row.
const TOP_VOTES_AT_LEAST: &str = "500";

/// How long since the User last watched a title before Watch it again offers it: any change to
/// its Watch State, or an Episode's, counts.
const WATCH_AGAIN_AFTER: Duration = Duration::from_hours(365 * 24);

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
/// If more than [`MOST_PICKS`] shared rows, or rows for each User, are picked, or TMDB refuses
/// the key or can't answer.
fn check_settings() -> Result<CheckedSettings, Error> {
    for (key, rows) in [
        (SHARED_ROWS, "shared rows"),
        (USER_ROWS, "rows for each User"),
    ] {
        let picked = nama_pdk::setting_picked(key)?.len();
        if picked > MOST_PICKS {
            return Err(Error::msg(format!(
                "pick at most {MOST_PICKS} {rows}, not {picked}"
            )));
        }
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
4. Pick up to 20 Shared rows, and up to 20 Your rows. Users hide or reorder them in their Home layout.
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
    let shared_rows = SettingField {
        default_value: setting_default(picked_choices([
            TRENDING,
            "top-rated-movies",
            "top-rated-shows",
        ])),
        ..choices_setting_field(
            SHARED_ROWS,
            "Shared rows",
            format!("The rows every User sees, at most {MOST_PICKS}."),
            SHARED,
        )
    };
    let user_rows = SettingField {
        default_value: setting_default(picked_choices([BECAUSE, MORE_GENRE, WATCHLIST])),
        ..choices_setting_field(
            USER_ROWS,
            "Your rows",
            format!(
                "The rows each User gets from what they watch and their Watchlist, at most \
                 {MOST_PICKS}."
            ),
            PER_USER,
        )
    };
    Manifest {
        name: "Discover".to_owned(),
        description: "Shows rows such as Trending this week, and each User's Watchlist, on the \
                      home page. This product uses the TMDB API but is not endorsed or certified \
                      by TMDB."
            .to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::HOME_ROW)],
        allowed_hosts: vec![
            "api.themoviedb.org".to_owned(),
            "query.wikidata.org".to_owned(),
        ],
        // Asked from the start, so an Admin grants once what Discover's rows for each User and its
        // award rows, kept in the Store, will need.
        permissions: vec![
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_WATCHLISTS.into(),
            Permission::PERMISSION_STORE.into(),
        ],
        settings: vec![api_key, region, shared_rows, user_rows],
        setup_markdown: SETUP.to_owned(),
        homepage_url: "https://www.themoviedb.org/".to_owned(),
        source_url: concat!(env!("CARGO_PKG_REPOSITORY"), "/tree/main/plugins/discover").to_owned(),
        license: env!("CARGO_PKG_LICENSE").to_owned(),
        ..nama_pdk::manifest("discover", env!("CARGO_PKG_VERSION"))
    }
}

/// The picked choices `values`, as a several-choices setting holds them.
fn picked_choices<const N: usize>(values: [&str; N]) -> setting_value::Value {
    Texts {
        values: values.map(str::to_owned).to_vec(),
        ..Texts::default()
    }
    .into()
}

/// Serves the rows the Admin picks.
struct Discover;

impl HomeRowSource for Discover {
    /// A row for each shared row picked, in [`SHARED`]'s order.
    ///
    /// # Errors
    /// If the API key isn't set, or TMDB or Wikidata can't answer for any row: a refresh shows
    /// every row or none, so the Kernel keeps the rows it had.
    fn shared_rows() -> Result<HomeRows, Error> {
        let api = Api::from_settings()?;
        let picked = nama_pdk::setting_picked(SHARED_ROWS)?;
        let mut rows = Vec::new();
        for (key, heading) in picked_rows(&SHARED, &picked) {
            // ponytail: a picked row Discover doesn't answer yet is left out until it's built.
            let titles = match key {
                TRENDING => api.titles(&["trending", "all", "week"])?,
                _ => match award_query(key) {
                    Some(query) => award_movies(key, &query)?,
                    None => continue,
                },
            };
            rows.push(row(key, heading, FRESH_FOR, titles)?);
        }
        Ok(HomeRows {
            rows,
            ..HomeRows::default()
        })
    }

    /// A row for each of the User's rows picked, in [`PER_USER`]'s order.
    ///
    /// # Errors
    /// If the Kernel refuses to list the Libraries or the User's Watch States, or, for a row from
    /// TMDB, the API key isn't set or TMDB can't answer, as for [`Self::shared_rows`].
    fn user_rows(asked: UserRowsAsked) -> Result<HomeRows, Error> {
        let picked = nama_pdk::setting_picked(USER_ROWS)?;
        let history = History::of(asked.user_id)?;
        let mut rows = Vec::new();
        for (key, heading) in picked_rows(&PER_USER, &picked) {
            // ponytail: a picked row Discover doesn't answer yet is left out until it's built.
            match key {
                BECAUSE => rows.extend(history.because_you_watched(&Api::from_settings()?)?),
                MORE_GENRE => rows.extend(history.more_genre(&Api::from_settings()?)?),
                WATCH_AGAIN => rows.push(row(
                    key,
                    heading,
                    USER_FRESH_FOR,
                    history.played_long_ago()?,
                )?),
                // The Admin may grant the other Plugin Permissions alone (ADR 0071): answer the
                // rows those allow.
                WATCHLIST => match kernel::watchlist(asked.user_id) {
                    Ok(watchlist) => rows.push(row(
                        key,
                        heading,
                        USER_FRESH_FOR,
                        history.unstarted(&watchlist),
                    )?),
                    Err(err) => nama_pdk::warn!("leaving out Your Watchlist: {err}"),
                },
                _ => {}
            }
        }
        Ok(HomeRows {
            rows,
            ..HomeRows::default()
        })
    }
}

nama_pdk::home_row_source!(Discover, [shared_rows, user_rows]);

/// The rows of `offered` whose key is `picked`, in `offered`'s order.
fn picked_rows<'a>(
    offered: &'a [(&'a str, &'a str)],
    picked: &'a [String],
) -> impl Iterator<Item = (&'a str, &'a str)> {
    (offered.iter().copied()).filter(|(key, _)| picked.iter().any(|pick| pick == key))
}

/// The row `key` headed `heading`, cut to the Kernel's [`MOST_HEADING_CHARS`] with "…" ending it,
/// since the Kernel cuts it without one.
fn row(
    key: &str,
    heading: &str,
    fresh_for: Duration,
    titles: Vec<HomeRowTitle>,
) -> Result<HomeRow, Error> {
    let heading = if heading.chars().count() > MOST_HEADING_CHARS {
        let kept: String = heading.chars().take(MOST_HEADING_CHARS - 1).collect();
        format!("{kept}…")
    } else {
        heading.to_owned()
    };
    Ok(HomeRow {
        key: key.to_owned(),
        heading,
        ttl_seconds: fresh_for.as_secs().try_into()?,
        titles,
        ..HomeRow::default()
    })
}

/// A Movie, or a Show by its Episodes' `show_id`, as a row names it.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    kind: TitleKind,
    namespace: String,
    value: String,
}

impl Key {
    fn of(kind: TitleKind, id: &TitleId) -> Self {
        Self {
            kind,
            namespace: id.namespace.clone(),
            value: id.value.clone(),
        }
    }

    fn tmdb(kind: TitleKind, id: u64) -> Self {
        Self {
            kind,
            namespace: "tmdb".to_owned(),
            value: id.to_string(),
        }
    }

    /// Where TMDB keeps the title, such as `["movie", "603"]`; `None` if it isn't named by its
    /// TMDB id.
    fn tmdb_path(&self) -> Option<[&str; 2]> {
        (self.namespace == "tmdb").then_some([media(self.kind)?, self.value.as_str()])
    }

    /// The Movie `title` is, or the Show whose Episode it is; `None` for an Episode without its
    /// Show's id.
    fn of_title(title: &Title) -> Option<Self> {
        let title_ref = title.title_ref.as_option()?;
        match title_ref.kind.as_known()? {
            TitleRefKind::TITLE_REF_KIND_MOVIE => Some(Self::of(
                TitleKind::TITLE_KIND_MOVIE,
                title_ref.id.as_option()?,
            )),
            TitleRefKind::TITLE_REF_KIND_EPISODE => Some(Self::of(
                TitleKind::TITLE_KIND_SHOW,
                title.show_id.as_option()?,
            )),
            TitleRefKind::TITLE_REF_KIND_UNSPECIFIED => None,
        }
    }

    fn row_title(&self) -> HomeRowTitle {
        HomeRowTitle {
            kind: self.kind.into(),
            id: TitleId {
                namespace: self.namespace.clone(),
                value: self.value.clone(),
                ..TitleId::default()
            }
            .into(),
            ..HomeRowTitle::default()
        }
    }
}

/// What one User did with each Movie and Show on the Server, in the order the Kernel lists them.
struct History {
    titles: Vec<Watched>,
}

/// One User's Watch State of a Movie, or of every Episode of a Show the Server has.
struct Watched {
    key: Key,
    /// The Movie's title, or the Show's.
    name: String,
    genres: Vec<String>,
    /// The Movie's 1, or the Show's Episodes the Server has.
    episodes: usize,
    played_episodes: usize,
    started: bool,
    /// Whether the User played the Movie, or an Episode, through or past halfway.
    played_or_past_halfway: bool,
    /// When the Watch State, or any Episode's, last changed, in milliseconds since the Unix epoch.
    last_changed_ms: i64,
}

impl History {
    /// The User `user`'s history of every title in every Library. A title whose Watch State isn't
    /// paired with it is left out.
    fn of(user: i64) -> Result<Self, Error> {
        let mut titles = Vec::new();
        for library in kernel::libraries()? {
            titles.extend(kernel::titles(library.id)?);
        }
        let refs: Vec<_> = (titles.iter())
            .map(|title| (*title.title_ref).clone())
            .collect();
        // ponytail: one call for every title on the Server, as Top Picks makes; ask a page at a
        // time if a large Server's answer outgrows the Plugin's memory.
        let states = kernel::watch_states(&refs, Some(user))?;
        let mut order = Vec::new();
        let mut watched: HashMap<Key, Watched> = HashMap::new();
        for (title, state) in titles.iter().zip(&states) {
            let Some(key) = Key::of_title(title).filter(|_| state.title_ref == title.title_ref)
            else {
                continue;
            };
            let entry = watched.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                Watched {
                    key,
                    name: (title.show_title.clone()).unwrap_or_else(|| title.title.clone()),
                    genres: title.genres.clone(),
                    episodes: 0,
                    played_episodes: 0,
                    started: false,
                    played_or_past_halfway: false,
                    last_changed_ms: 0,
                }
            });
            entry.add(&state.states);
        }
        Ok(Self {
            titles: (order.iter())
                .filter_map(|key| watched.remove(key))
                .collect(),
        })
    }

    /// The titles of `watchlist` the User hasn't started or played, in its order.
    fn unstarted(&self, watchlist: &[WatchlistTitle]) -> Vec<HomeRowTitle> {
        self.unstarted_of(
            (watchlist.iter())
                .filter_map(|title| Some(Key::of(title.kind.as_known()?, title.id.as_option()?))),
        )
    }

    /// The titles of `keys` the User hasn't started or played, in their order.
    ///
    /// ponytail: a title matches only by the id the Kernel lists it under, so one named by
    /// another, such as a Show listed by its TVDB id, shows even once started; match every id
    /// when the Kernel lists them (#1717).
    fn unstarted_of(&self, keys: impl IntoIterator<Item = Key>) -> Vec<HomeRowTitle> {
        let started: HashSet<&Key> = (self.titles.iter())
            .filter(|title| title.started)
            .map(|title| &title.key)
            .collect();
        (keys.into_iter())
            .filter(|key| !started.contains(key))
            .map(|key| key.row_title())
            .collect()
    }

    /// A row of TMDB's recommendations for each of the [`BECAUSE_ROWS`] titles the User played
    /// most recently, keyed by its place, `because-1` first, so a row keeps its place in the
    /// User's Home layout as the title it follows changes.
    ///
    /// ponytail: a title the Kernel names by another id, such as a Show by its TVDB id, is
    /// skipped until the Kernel lists every id of a title (#1717).
    ///
    /// # Errors
    /// If TMDB can't answer.
    fn because_you_watched(&self, api: &Api) -> Result<Vec<HomeRow>, Error> {
        let mut played: Vec<(&Watched, [&str; 2])> = (self.titles.iter())
            .filter(|title| title.played_or_past_halfway)
            .filter_map(|title| Some((title, title.key.tmdb_path()?)))
            .collect();
        played.sort_by_key(|(title, _)| Reverse(title.last_changed_ms));
        (played.into_iter().take(BECAUSE_ROWS).enumerate())
            .map(|(place, (title, [media, id]))| {
                let recommended =
                    api.titles_of(title.key.kind, &[media, id, "recommendations"], &[])?;
                row(
                    &format!("{BECAUSE}-{}", place + 1),
                    &format!("Because you watched {}", title.name),
                    USER_FRESH_FOR,
                    self.unstarted_of(recommended),
                )
            })
            .collect()
    }

    /// The titles the User played through and last watched over [`WATCH_AGAIN_AFTER`] ago, those
    /// of the genre they played most first.
    fn played_long_ago(&self) -> Result<Vec<HomeRowTitle>, Error> {
        let long_ago = nama_pdk::now() - WATCH_AGAIN_AFTER;
        let long_ago_ms = i64::try_from(long_ago.duration_since(UNIX_EPOCH)?.as_millis())?;
        let genre_plays = self.genre_plays();
        let mut again: Vec<&Watched> = (self.titles.iter())
            .filter(|title| {
                title.played_episodes == title.episodes && title.last_changed_ms < long_ago_ms
            })
            .collect();
        again.sort_by_key(|title| {
            let plays = (title.genres.iter()).filter_map(|genre| genre_plays.get(genre.as_str()));
            Reverse(plays.max())
        });
        Ok(again.iter().map(|title| title.key.row_title()).collect())
    }

    /// How many titles of each genre the User played an Episode of, or the Movie.
    fn genre_plays(&self) -> HashMap<&str, usize> {
        let mut plays = HashMap::new();
        for genre in (self.titles.iter().filter(|title| title.played_episodes > 0))
            .flat_map(|title| &title.genres)
        {
            *plays.entry(genre.as_str()).or_default() += 1;
        }
        plays
    }

    /// A row of TMDB's top Movies and Shows in each of the [`GENRE_ROWS`] genres the User played
    /// most, keyed by its place as [`Self::because_you_watched`]'s are. A genre TMDB has no
    /// genre of that name for, Movie or Show, is passed over for the next.
    ///
    /// ponytail: genres match by name, and the Kernel names them in the title source's language
    /// while TMDB's lists are in its default one, so a Server whose titles aren't in English gets
    /// no More rows; ask TMDB's lists in that language once the Kernel tells a Plugin which
    /// (#1718).
    ///
    /// # Errors
    /// If TMDB can't answer.
    fn more_genre(&self, api: &Api) -> Result<Vec<HomeRow>, Error> {
        let movie_genres = api.genres(TitleKind::TITLE_KIND_MOVIE)?;
        let show_genres = api.genres(TitleKind::TITLE_KIND_SHOW)?;
        let mut plays: Vec<_> = self.genre_plays().into_iter().collect();
        plays.sort_by_key(|&(genre, plays)| (Reverse(plays), genre));
        (plays.into_iter())
            .map(|(genre, _)| (genre, movie_genres.get(genre), show_genres.get(genre)))
            .filter(|(_, movie_genre, show_genre)| movie_genre.is_some() || show_genre.is_some())
            .take(GENRE_ROWS)
            .enumerate()
            .map(|(place, (genre, movie_genre, show_genre))| {
                let movies = api.top_in(TitleKind::TITLE_KIND_MOVIE, movie_genre)?;
                let shows = api.top_in(TitleKind::TITLE_KIND_SHOW, show_genre)?;
                row(
                    &format!("{MORE_GENRE}-{}", place + 1),
                    &format!("More {genre}"),
                    USER_FRESH_FOR,
                    self.unstarted_of(interleave(movies, shows)),
                )
            })
            .collect()
    }
}

/// `a`'s and `b`'s keys taking turns, `a`'s first, and the longer's rest after.
fn interleave(a: Vec<Key>, b: Vec<Key>) -> Vec<Key> {
    let (mut a, mut b) = (a.into_iter(), b.into_iter());
    let mut both = Vec::new();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return both,
            (from_a, from_b) => both.extend(from_a.into_iter().chain(from_b)),
        }
    }
}

impl Watched {
    /// Adds `states`, the User's Watch State of the Movie or of one of the Show's Episodes.
    fn add(&mut self, states: &[WatchState]) {
        let played = states.iter().any(|state| state.played);
        self.episodes += 1;
        self.played_episodes += usize::from(played);
        self.started |= played || states.iter().any(|state| state.position_ms > 0);
        self.played_or_past_halfway |= played
            || (states.iter())
                .any(|state| state.duration_ms > 0 && state.position_ms > state.duration_ms / 2);
        self.last_changed_ms = (states.iter())
            .map(|state| state.updated_at_ms)
            .fold(self.last_changed_ms, i64::max);
    }
}

/// How TMDB's paths name `kind`; `None` for neither a Movie nor a Show.
fn media(kind: TitleKind) -> Option<&'static str> {
    match kind {
        TitleKind::TITLE_KIND_MOVIE => Some("movie"),
        TitleKind::TITLE_KIND_SHOW => Some("tv"),
        TitleKind::TITLE_KIND_UNSPECIFIED => None,
    }
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

/// TMDB's genres of Movies, or of Shows.
#[derive(Deserialize)]
struct GenreList {
    genres: Vec<Genre>,
}

#[derive(Deserialize)]
struct Genre {
    id: u64,
    name: String,
}

/// One title in a page, or a person in a trending one.
#[derive(Deserialize)]
struct Listing {
    id: u64,
    /// Unset in a page of one media, such as a Movie's recommendations.
    #[serde(default)]
    media_type: String,
}

impl Api {
    fn from_settings() -> Result<Self, Error> {
        Ok(Self {
            key: nama_pdk::required_setting(API_KEY)?.trim().to_owned(),
        })
    }

    /// The Movies and Shows in the first page at `path`, leaving out people.
    // ponytail: one page, TMDB's 20 titles; ask for more pages if a row of 20 feels short.
    fn titles(&self, path: &[&str]) -> Result<Vec<HomeRowTitle>, Error> {
        let page: Page = self.get(path, &[])?;
        Ok(page.results.iter().filter_map(title).collect())
    }

    /// The titles in the first page at `path` asked with `query`, a page of `kind` alone.
    fn titles_of(
        &self,
        kind: TitleKind,
        path: &[&str],
        query: &[(&str, &str)],
    ) -> Result<Vec<Key>, Error> {
        let page: Page = self.get(path, query)?;
        Ok((page.results.iter())
            .map(|listing| Key::tmdb(kind, listing.id))
            .collect())
    }

    /// TMDB's genres of `kind`, by name.
    fn genres(&self, kind: TitleKind) -> Result<HashMap<String, u64>, Error> {
        let list: GenreList = self.get(&["genre", media(kind).unwrap_or_default(), "list"], &[])?;
        Ok((list.genres.into_iter())
            .map(|genre| (genre.name, genre.id))
            .collect())
    }

    /// TMDB's top rated titles of `kind` in its genre `genre`; none for no genre.
    fn top_in(&self, kind: TitleKind, genre: Option<&u64>) -> Result<Vec<Key>, Error> {
        let Some(genre) = genre else {
            return Ok(Vec::new());
        };
        self.titles_of(
            kind,
            &["discover", media(kind).unwrap_or_default()],
            &[
                ("with_genres", &genre.to_string()),
                ("sort_by", "vote_average.desc"),
                ("vote_count.gte", TOP_VOTES_AT_LEAST),
            ],
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

/// The Home row title of `listing`, or `None` for a person.
fn title(listing: &Listing) -> Option<HomeRowTitle> {
    let kind = match listing.media_type.as_str() {
        "movie" => TitleKind::TITLE_KIND_MOVIE,
        "tv" => TitleKind::TITLE_KIND_SHOW,
        _ => return None,
    };
    Some(Key::tmdb(kind, listing.id).row_title())
}

/// The SPARQL query for the Movies of the award row `key`, as TMDB ids newest first, or `None`
/// when `key` isn't an award row.
fn award_query(key: &str) -> Option<String> {
    Some(match key {
        "best-picture" => winners_of(BEST_PICTURE),
        "oscar-nominees" => format!(
            "SELECT ?tmdb (MAX(?date) AS ?newest) WHERE {{ \
             ?movie p:P1411 ?nomination. ?nomination ps:P1411 wd:{BEST_PICTURE}; pq:P585 ?date. \
             FILTER(YEAR(?date) > YEAR(NOW()) - {NOMINEE_YEARS}) \
             FILTER NOT EXISTS {{ ?movie wdt:P166 wd:{BEST_PICTURE} }} \
             ?movie wdt:P4947 ?tmdb. \
             }} GROUP BY ?tmdb ORDER BY DESC(?newest)"
        ),
        "palme-dor" => winners_of(PALME_DOR),
        "golden-globe-dramas" => winners_of(GOLDEN_GLOBE_DRAMA),
        _ => return None,
    })
}

/// Wikidata's items for the awards Discover has rows of.
const BEST_PICTURE: &str = "Q102427";
const PALME_DOR: &str = "Q179808";
const GOLDEN_GLOBE_DRAMA: &str = "Q1011509";

/// How many years of Best Picture nominees Oscar nominees holds.
const NOMINEE_YEARS: u32 = 10;

/// The SPARQL query for the Movies that won the Wikidata item `award`, as TMDB ids newest first.
/// Wikidata leaves some wins undated, so those go by the Movie's first release: it lists a release
/// per country and re-release, so a later one would sort a 1973 winner as 1975.
fn winners_of(award: &str) -> String {
    format!(
        "SELECT ?tmdb (COALESCE(MAX(?awarded), MIN(?released)) AS ?newest) WHERE {{ \
         ?movie p:P166 ?won. ?won ps:P166 wd:{award}. ?movie wdt:P4947 ?tmdb. \
         OPTIONAL {{ ?won pq:P585 ?awarded }} OPTIONAL {{ ?movie wdt:P577 ?released }} \
         }} GROUP BY ?tmdb ORDER BY DESC(?newest)"
    )
}

/// The Movies of the award row `key`, which Wikidata answers `query` with, in its order. The Store
/// keeps them for [`AWARDS_FRESH_FOR`]; without it Discover asks each refresh (ADR 0071).
///
/// # Errors
/// If they aren't kept fresh and Wikidata can't answer, or the Server's clock is before 1970.
fn award_movies(key: &str, query: &str) -> Result<Vec<HomeRowTitle>, Error> {
    let store_key = format!("award:{key}");
    let now = nama_pdk::now().duration_since(UNIX_EPOCH)?.as_secs();
    let movies = if let Some(fresh) = fresh_movies(&store_key, now) {
        fresh
    } else {
        let asked = KeptMovies {
            asked_at: now,
            movies: ask_wikidata(query)?,
        };
        keep(&store_key, &asked);
        asked.movies
    };
    Ok((movies.into_iter())
        .map(|id| Key::tmdb(TitleKind::TITLE_KIND_MOVIE, id).row_title())
        .collect())
}

/// The Movies the Store keeps under `store_key`, or `None` when it keeps none Wikidata answered
/// within [`AWARDS_FRESH_FOR`] of `now`, or isn't granted.
fn fresh_movies(store_key: &str, now: u64) -> Option<Vec<u64>> {
    let kept: KeptMovies = json::from_slice(&kernel::store_get(store_key, None).ok()??).ok()?;
    (now.saturating_sub(kept.asked_at) < AWARDS_FRESH_FOR.as_secs()).then_some(kept.movies)
}

/// Keeps `asked` in the Store under `store_key`. Unkept, the next refresh asks Wikidata again,
/// which is all it costs (ADR 0071: the Plugin carries on).
fn keep(store_key: &str, asked: &KeptMovies) {
    if let Ok(asked) = json::to_vec(asked) {
        let _not_kept: Result<(), Error> = kernel::store_put(store_key, None, &asked);
    }
}

/// Wikidata's answer to `query`: the TMDB ids it binds to `?tmdb`, in its order.
///
/// # Errors
/// If Wikidata didn't answer, refused, or answered with something unexpected.
fn ask_wikidata(query: &str) -> Result<Vec<u64>, Error> {
    let mut url = Url::parse(SPARQL_URL)?;
    url.query_pairs_mut().append_pair("query", query);
    let request = HttpRequest::new(url.as_str())
        .with_header("Accept", "application/sparql-results+json")
        .with_header("User-Agent", USER_AGENT);
    let response = WIKIDATA.send(&request, None::<String>)?;
    let answer: Sparql = json::from_slice(WIKIDATA.accepted(response)?.body()).map_err(|err| {
        Error::msg(format!(
            "Wikidata answered with something unexpected: {err}"
        ))
    })?;
    Ok((answer.results.bindings.into_iter())
        .filter_map(|movie| movie.tmdb?.value.parse().ok())
        .collect())
}

/// An award row's Movies as the Store keeps them, by TMDB id, with when Wikidata answered them in
/// seconds since the Unix epoch.
#[derive(Deserialize, Serialize)]
struct KeptMovies {
    asked_at: u64,
    movies: Vec<u64>,
}

/// A SPARQL query's answer, in the JSON format W3C sets.
#[derive(Deserialize)]
struct Sparql {
    results: Bindings,
}

#[derive(Deserialize)]
struct Bindings {
    bindings: Vec<AwardMovie>,
}

/// A Movie in an award query's answer.
#[derive(Deserialize)]
struct AwardMovie {
    /// `None`, or not a number, for a Movie Wikidata has no TMDB id for.
    tmdb: Option<Literal>,
}

#[derive(Deserialize)]
struct Literal {
    value: String,
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
