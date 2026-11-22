//! The Discover Installed Plugin: a Home row source of rows every User sees, such as Trending this
//! week, from TMDB, and of each User's own, such as their Watchlist, the Admin picking which it
//! offers. The Server builds it for wasm32 and bundles it in its binary.

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

const STARRING: &str = "starring";
const FROM: &str = "from";
const WATCH_AGAIN: &str = "watch-again";
const WATCHLIST: &str = "watchlist";

/// Every row Discover offers each User, by key and heading, as [`SHARED`] lists its rows. Because
/// you watched and More genre each stand for several rows, headed by what they follow.
const PER_USER: [(&str, &str); 6] = [
    ("because", "Because you watched"),
    ("more-genre", "More genre"),
    (STARRING, "Starring"),
    (FROM, "From"),
    (WATCH_AGAIN, "Watch it again"),
    (WATCHLIST, "Your Watchlist"),
];

/// How long a User's row stays fresh. The Kernel asks again sooner once they play something.
const USER_FRESH_FOR: Duration = Duration::from_hours(6);

/// How many of a title's Cast, by billing, count towards a Starring row, so minor cast who
/// happen to appear in several titles never get one.
const TOP_BILLED: u32 = 5;

/// The most characters the Kernel keeps of a heading (ADR 0058).
const MOST_HEADING_CHARACTERS: usize = 40;

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
        default_value: setting_default(picked_choices(["because", "more-genre", WATCHLIST])),
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
        allowed_hosts: vec!["api.themoviedb.org".to_owned()],
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
    /// If the API key isn't set, or TMDB can't answer for any row: a refresh shows every row or
    /// none, so the Kernel keeps the rows it had.
    fn shared_rows() -> Result<HomeRows, Error> {
        let api = Api::from_settings()?;
        let picked = nama_pdk::setting_picked(SHARED_ROWS)?;
        let mut rows = Vec::new();
        for (key, heading) in picked_rows(&SHARED, &picked) {
            // ponytail: a picked row Discover doesn't answer yet is left out until it's built.
            let titles = match key {
                TRENDING => api.titles(&["trending", "all", "week"])?,
                _ => continue,
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
    /// If the Kernel refuses to list the Libraries or the User's Watch States, or, with Starring
    /// or From picked, TMDB can't answer.
    fn user_rows(asked: UserRowsAsked) -> Result<HomeRows, Error> {
        let picked = nama_pdk::setting_picked(USER_ROWS)?;
        let history = History::of(asked.user_id)?;
        let people = if picked.iter().any(|pick| pick == STARRING || pick == FROM) {
            People::of(&history)?
        } else {
            People::default()
        };
        let mut rows = Vec::new();
        for (key, heading) in picked_rows(&PER_USER, &picked) {
            // ponytail: a picked row Discover doesn't answer yet is left out until it's built.
            let (heading, titles) = match key {
                WATCH_AGAIN => (heading.to_owned(), history.played_long_ago()?),
                // The Admin may grant the other Plugin Permissions alone (ADR 0071): answer the
                // rows those allow.
                WATCHLIST => match kernel::watchlist(asked.user_id) {
                    Ok(watchlist) => (heading.to_owned(), history.unstarted(&watchlist)),
                    Err(err) => {
                        nama_pdk::warn!("leaving out Your Watchlist: {err}");
                        continue;
                    }
                },
                STARRING | FROM => match people.row(key, heading, &history)? {
                    Some(row) => row,
                    None => continue,
                },
                _ => continue,
            };
            rows.push(row(key, &heading, USER_FRESH_FOR, titles)?);
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

fn row(
    key: &str,
    heading: &str,
    fresh_for: Duration,
    titles: Vec<HomeRowTitle>,
) -> Result<HomeRow, Error> {
    Ok(HomeRow {
        key: key.to_owned(),
        heading: heading.to_owned(),
        ttl_seconds: fresh_for.as_secs().try_into()?,
        titles,
        ..HomeRow::default()
    })
}

/// `heading`, cut with "…" to [`MOST_HEADING_CHARACTERS`].
fn cut(heading: String) -> String {
    if heading.chars().count() <= MOST_HEADING_CHARACTERS {
        return heading;
    }
    (heading.chars().take(MOST_HEADING_CHARACTERS - 1))
        .chain(['…'])
        .collect()
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
    genres: Vec<String>,
    /// The Movie's 1, or the Show's Episodes the Server has.
    episodes: usize,
    played_episodes: usize,
    started: bool,
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
                    genres: title.genres.clone(),
                    episodes: 0,
                    played_episodes: 0,
                    started: false,
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

    /// The titles the User played, of a Movie or of any Episode of a Show.
    fn played(&self) -> impl Iterator<Item = &Watched> {
        self.titles.iter().filter(|title| title.played_episodes > 0)
    }

    /// `titles` but those the User played, each once, in their order.
    ///
    /// ponytail: a played title matches only by the id the Kernel lists it under, as in
    /// [`Self::unstarted`].
    fn unplayed(&self, titles: Vec<Key>) -> Vec<HomeRowTitle> {
        let mut shown: HashSet<Key> = self.played().map(|title| title.key.clone()).collect();
        (titles.into_iter())
            .filter(|key| shown.insert(key.clone()))
            .map(|key| key.row_title())
            .collect()
    }

    /// The titles of `watchlist` the User hasn't started or played, in its order.
    ///
    /// ponytail: a Watchlist title matches only by the id the Kernel lists it under, so one added
    /// under another id shows even once started; match every id when the Kernel lists them.
    fn unstarted(&self, watchlist: &[WatchlistTitle]) -> Vec<HomeRowTitle> {
        let started: HashSet<&Key> = (self.titles.iter())
            .filter(|title| title.started)
            .map(|title| &title.key)
            .collect();
        (watchlist.iter())
            .filter_map(|title| Some(Key::of(title.kind.as_known()?, title.id.as_option()?)))
            .filter(|key| !started.contains(key))
            .map(|key| key.row_title())
            .collect()
    }

    /// The titles the User played through and last watched over [`WATCH_AGAIN_AFTER`] ago, those
    /// of the genre they played most first.
    fn played_long_ago(&self) -> Result<Vec<HomeRowTitle>, Error> {
        let long_ago = nama_pdk::now() - WATCH_AGAIN_AFTER;
        let long_ago_ms = i64::try_from(long_ago.duration_since(UNIX_EPOCH)?.as_millis())?;
        let mut genre_plays: HashMap<&str, usize> = HashMap::new();
        for genre in self.played().flat_map(|title| &title.genres) {
            *genre_plays.entry(genre).or_default() += 1;
        }
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
}

impl Watched {
    /// Adds `states`, the User's Watch State of the Movie or of one of the Show's Episodes.
    fn add(&mut self, states: &[WatchState]) {
        let played = states.iter().any(|state| state.played);
        self.episodes += 1;
        self.played_episodes += usize::from(played);
        self.started |= played || states.iter().any(|state| state.position_ms > 0);
        self.last_changed_ms = (states.iter())
            .map(|state| state.updated_at_ms)
            .fold(self.last_changed_ms, i64::max);
    }
}

/// Who a User's played titles credit, most recently played first, and each in billing order.
#[derive(Default)]
struct People {
    credits: Vec<Credits>,
}

/// What a person did on a title.
#[derive(Clone, Copy)]
enum Role {
    /// Billed in the first [`TOP_BILLED`] of its Cast.
    Cast,
    /// Its Director, or a Show's Creator.
    Maker,
}

impl People {
    /// The credits of every title the User of `history` played that has a TMDB id.
    ///
    /// ponytail: asks TMDB once per played title not yet in the Store, so a User's first refresh
    /// on a large Server makes many calls; count only their latest titles if that proves slow.
    fn of(history: &History) -> Result<Self, Error> {
        let api = Api::from_settings()?;
        let mut played: Vec<&Watched> = (history.played())
            .filter(|title| title.key.namespace == "tmdb")
            .collect();
        played.sort_by_key(|title| Reverse(title.last_changed_ms));
        let credits = (played.iter())
            .map(|title| Credits::of(&api, &title.key))
            .collect::<Result<_, _>>()?;
        Ok(Self { credits })
    }

    /// The heading and titles of the row `key`, Starring or From, its `heading` naming who it
    /// follows; `None` if no title the User of `history` played credits anyone so.
    fn row(
        &self,
        key: &str,
        heading: &str,
        history: &History,
    ) -> Result<Option<(String, Vec<HomeRowTitle>)>, Error> {
        let role = if key == STARRING {
            Role::Cast
        } else {
            Role::Maker
        };
        let Some(person) = self.most_credited(role) else {
            return Ok(None);
        };
        let titles = Api::from_settings()?.titles_of(person, role)?;
        let heading = cut(format!("{heading} {}", person.name));
        Ok(Some((heading, history.unplayed(titles))))
    }

    /// The person credited as `role` in the most titles; of as many, the one credited first.
    fn most_credited(&self, role: Role) -> Option<&Person> {
        let credited = (self.credits.iter()).flat_map(|credits| match role {
            Role::Cast => &credits.cast,
            Role::Maker => &credits.makers,
        });
        let mut tally: HashMap<u64, (usize, Reverse<usize>, &Person)> = HashMap::new();
        for (first, person) in credited.enumerate() {
            tally
                .entry(person.id)
                .or_insert((0, Reverse(first), person))
                .0 += 1;
        }
        (tally.into_values())
            .max_by_key(|(titles, first, _)| (*titles, *first))
            .map(|(_, _, person)| person)
    }
}

/// Who one title credits, each once, as the Store keeps them.
#[derive(Serialize, Deserialize)]
struct Credits {
    /// Its first [`TOP_BILLED`] Cast.
    cast: Vec<Person>,
    /// Its Directors and, for a Show, its Creators.
    makers: Vec<Person>,
}

/// A person, by TMDB's id.
#[derive(Clone, Serialize, Deserialize)]
struct Person {
    id: u64,
    name: String,
}

/// A Movie's or Show's details, with its credits, as TMDB answers them.
#[derive(Deserialize)]
struct Details {
    /// A Show's Creators; a Movie has none.
    #[serde(default)]
    created_by: Vec<Person>,
    credits: CreditLists,
}

#[derive(Deserialize)]
struct CreditLists {
    cast: Vec<Billed>,
    crew: Vec<Job>,
}

#[derive(Deserialize)]
struct Billed {
    #[serde(flatten)]
    person: Person,
    order: u32,
}

#[derive(Deserialize)]
struct Job {
    #[serde(flatten)]
    person: Person,
    job: String,
}

impl Credits {
    /// `title`'s credits, from the Store, or else from TMDB and then kept in the Store, which
    /// every User shares, so a refresh asks only for titles played since.
    fn of(api: &Api, title: &Key) -> Result<Self, Error> {
        let media = match title.kind {
            TitleKind::TITLE_KIND_SHOW => "tv",
            TitleKind::TITLE_KIND_UNSPECIFIED | TitleKind::TITLE_KIND_MOVIE => "movie",
        };
        let store_key = format!("credits/{media}/{}", title.value);
        // A Store not granted only costs each refresh its calls.
        if let Some(kept) = (kernel::store_get(&store_key, None).ok().flatten())
            .and_then(|kept| json::from_slice(&kept).ok())
        {
            return Ok(kept);
        }
        let details: Details =
            api.get(&[media, &title.value], &[("append_to_response", "credits")])?;
        let credits = Self::from(details);
        let _not_kept: Result<(), Error> =
            kernel::store_put(&store_key, None, &json::to_vec(&credits)?);
        Ok(credits)
    }
}

impl From<Details> for Credits {
    fn from(details: Details) -> Self {
        let cast = (details.credits.cast.into_iter())
            .filter(|billed| billed.order < TOP_BILLED)
            .map(|billed| billed.person);
        let directors = (details.credits.crew.into_iter())
            .filter(|job| job.job == "Director")
            .map(|job| job.person);
        Self {
            cast: each_once(cast),
            makers: each_once(directors.chain(details.created_by)),
        }
    }
}

/// `people`, each once, in their order: TMDB credits one person in several roles.
fn each_once(people: impl Iterator<Item = Person>) -> Vec<Person> {
    let mut seen = HashSet::new();
    people.filter(|person| seen.insert(person.id)).collect()
}

/// The Shows a person is in the cast or crew of, as TMDB answers them.
#[derive(Deserialize)]
struct ShowCredits {
    cast: Vec<Id>,
    crew: Vec<Id>,
}

/// A title in a list that says what kind it is by where it is.
#[derive(Deserialize)]
struct Id {
    id: u64,
}

/// TMDB's API, asked with the Admin's key.
struct Api {
    key: String,
}

/// A page of TMDB's titles.
#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
}

/// One title in a page, or a person in a trending one.
#[derive(Deserialize)]
struct Listing {
    id: u64,
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
        let page: Page<Listing> = self.get(path, &[])?;
        Ok(page.results.iter().filter_map(title).collect())
    }

    /// The Movies TMDB finds `person` in as `role`, then the Shows of their TV credits as it.
    ///
    /// ponytail: one page of Movies, TMDB's 20 most popular, as [`Self::titles`] asks.
    fn titles_of(&self, person: &Person, role: Role) -> Result<Vec<Key>, Error> {
        let id = person.id.to_string();
        let by = match role {
            Role::Cast => "with_cast",
            Role::Maker => "with_crew",
        };
        let movies: Page<Id> = self.get(&["discover", "movie"], &[(by, &id)])?;
        let shows: ShowCredits = self.get(&["person", &id, "tv_credits"], &[])?;
        let shows = match role {
            Role::Cast => shows.cast,
            Role::Maker => shows.crew,
        };
        let movies =
            (movies.results.iter()).map(|movie| Key::tmdb(TitleKind::TITLE_KIND_MOVIE, movie.id));
        let shows = (shows.iter()).map(|show| Key::tmdb(TitleKind::TITLE_KIND_SHOW, show.id));
        Ok(movies.chain(shows).collect())
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
            .append_pair("api_key", &self.key)
            .extend_pairs(query);
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

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
