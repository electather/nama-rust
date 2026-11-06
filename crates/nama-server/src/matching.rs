//! Matching: linking the items a scan found to titles from the metadata Provider, and storing
//! those titles' metadata and external ids on the items.
//!
//! After each scan, Matching decides about every item of the Library it hasn't decided yet:
//!
//! - An item whose name has an ID hint is fetched by that id directly. If the metadata source
//!   has no such title, the item is Unmatched.
//! - Otherwise its title and year are searched for, and it Matches only a result that fits
//!   confidently and unambiguously (see [`confidence`]). Anything else is Unmatched.
//! - An Episode Matches once its Show does and the Show's Season lists it. Each Show is looked up
//!   once per pass, and each of its Seasons fetched once. An Episode keeps its own TMDB id and its
//!   Show's external ids; Watch State keys Episodes by the Show's ids plus Season and Episode
//!   numbers (ADR 0008), so its own IMDb and TVDB ids aren't fetched.
//!
//! A Matched or Unmatched item stays so on later scans until its file changes. An item Matching
//! couldn't decide, because no metadata Provider is enabled or the Provider failed (such as when
//! TMDB is unreachable or rejects the API key), has no Match and so is Unmatched too, but nothing
//! is stored for it: it is undecided, and the next scan tries it again. Once the Provider fails,
//! the rest of that pass is left for the next scan too.

mod confidence;

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use nama_plugin_api::{
    PluginId,
    metadata::{
        Episode, ExternalId, ExternalIds, MetadataError, MetadataProvider, Movie, SearchQuery,
        Season, Show, TitleKind,
    },
};
use sqlx::SqliteConnection;
use tokio_util::sync::CancellationToken;

use crate::{db::Database, plugins::Registry, scanner::ItemKind};

use confidence::Decision;

/// What one pass of Matching decided.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    /// Items that now have a Match.
    pub(crate) matched: u64,
    /// Items that are now Unmatched.
    pub(crate) unmatched: u64,
    /// Items left undecided, to try again on the next scan.
    pub(crate) undecided: u64,
}

/// Why Matching stopped.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// The Server is shutting down.
    #[error("matching was cancelled")]
    Cancelled,
}

/// Matches the items of Libraries using the enabled metadata Provider.
#[derive(Debug, Clone)]
pub(crate) struct Matcher {
    db: Arc<Database>,
    plugins: Registry,
}

/// The title an item names: a Movie, or the Show an Episode is in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TitleKey {
    title: String,
    year: Option<i64>,
    /// From an ID hint.
    tmdb_id: Option<i64>,
}

/// An Episode waiting to be Matched.
#[derive(Debug, Clone, Copy)]
struct PendingEpisode {
    item: i64,
    season: i64,
    episode: i64,
}

/// Why Matching found no title for an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Miss {
    /// Nothing fits: the item is Unmatched.
    Unmatched,
    /// The metadata Provider couldn't answer: the item is Unmatched for now, but undecided, so
    /// the next scan tries again.
    Undecided,
}

/// What Matching decided about one item.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Decided {
    Matched(Box<Metadata>),
    Unmatched,
}

/// The metadata stored with a Match, as a row of `item_matches`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Metadata {
    /// The Plugin whose metadata Provider made the Match.
    plugin: PluginId,
    ids: ExternalIds,
    /// For an Episode, its Show's.
    show_ids: ExternalIds,
    title: String,
    original_title: Option<String>,
    show_title: Option<String>,
    overview: Option<String>,
    release_date: Option<String>,
    runtime_minutes: Option<u32>,
    poster: Option<String>,
    backdrop: Option<String>,
}

impl Metadata {
    fn movie(plugin: PluginId, movie: Movie) -> Self {
        Self {
            plugin,
            ids: movie.ids,
            show_ids: ExternalIds::default(),
            title: movie.title,
            original_title: movie.original_title,
            show_title: None,
            overview: movie.overview,
            release_date: movie.release_date,
            runtime_minutes: movie.runtime_minutes,
            poster: movie.poster,
            backdrop: movie.backdrop,
        }
    }

    fn episode(plugin: &PluginId, show: &Show, season: &Season, episode: &Episode) -> Self {
        Self {
            plugin: plugin.clone(),
            ids: episode.ids.clone(),
            show_ids: show.ids.clone(),
            title: episode.title.clone(),
            original_title: None,
            show_title: Some(show.title.clone()),
            overview: episode.overview.clone(),
            release_date: episode.air_date.clone(),
            runtime_minutes: episode.runtime_minutes,
            poster: season.poster.clone().or_else(|| show.poster.clone()),
            backdrop: show.backdrop.clone(),
        }
    }
}

impl Matcher {
    /// A matcher storing Matches in `db`, asking the metadata Provider enabled in `plugins`.
    pub(crate) const fn new(db: Arc<Database>, plugins: Registry) -> Self {
        Self { db, plugins }
    }

    /// Matches the items of the Library `library` that Matching hasn't decided yet, until
    /// `cancel` is cancelled. The caller holds the Library's scan lock, so its items don't change
    /// meanwhile.
    pub(crate) async fn match_library(
        &self,
        library: i64,
        cancel: &CancellationToken,
    ) -> Result<Summary, Error> {
        let (movies, shows) = self.undecided(library).await?;
        let mut summary = Summary::default();
        // Once the Provider fails (or there is none), asking about the other titles would only
        // fail too, so the rest is left for the next scan.
        let mut available = true;
        for (key, items) in movies {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if !available {
                summary.undecided += count(items.len());
                continue;
            }
            let outcome = self.match_movie(&key).await;
            available = !matches!(outcome, Err(Miss::Undecided));
            let decided = items.into_iter().map(|item| (item, outcome.clone()));
            self.store(decided.collect(), &mut summary).await?;
        }
        for (key, episodes) in shows {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if !available {
                summary.undecided += count(episodes.len());
                continue;
            }
            let decided = self.match_episodes(&key, &episodes).await;
            available = !decided
                .iter()
                .any(|(_, outcome)| matches!(outcome, Err(Miss::Undecided)));
            self.store(decided, &mut summary).await?;
        }
        if summary != Summary::default() {
            tracing::info!(library, ?summary, "matched a Library");
        }
        if !available {
            tracing::warn!(
                library,
                "no metadata provider could answer; will try again next scan"
            );
        }
        Ok(summary)
    }

    /// The Library's undecided Movies and Episodes, grouped by the title they name.
    async fn undecided(
        &self,
        library: i64,
    ) -> Result<
        (
            BTreeMap<TitleKey, Vec<i64>>,
            BTreeMap<TitleKey, Vec<PendingEpisode>>,
        ),
        sqlx::Error,
    > {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_items.kind AS "kind: ItemKind",
                      library_items.title, library_items.year, library_items.tmdb_id,
                      library_items.season, library_items.episode
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ?
                 AND NOT EXISTS (SELECT 1 FROM item_matches
                                 WHERE item_matches.item_id = library_items.id)
               ORDER BY library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut movies: BTreeMap<TitleKey, Vec<i64>> = BTreeMap::new();
        let mut shows: BTreeMap<TitleKey, Vec<PendingEpisode>> = BTreeMap::new();
        for row in rows {
            let key = TitleKey {
                title: row.title,
                year: row.year,
                tmdb_id: row.tmdb_id,
            };
            match (row.kind, row.season, row.episode) {
                (ItemKind::Episode, Some(season), Some(episode)) => {
                    shows.entry(key).or_default().push(PendingEpisode {
                        item: row.id,
                        season,
                        episode,
                    });
                }
                (ItemKind::Movie | ItemKind::Episode, _, _) => {
                    movies.entry(key).or_default().push(row.id);
                }
            }
        }
        Ok((movies, shows))
    }

    /// Finds and fetches the Movie `key` names.
    async fn match_movie(&self, key: &TitleKey) -> Result<Metadata, Miss> {
        let id = self.find(TitleKind::Movie, key).await?;
        let (plugin, movie) = self
            .ask(|metadata| {
                let id = id.clone();
                async move { metadata.movie(&id).await }
            })
            .await?;
        Ok(Metadata::movie(plugin, movie))
    }

    /// Finds and fetches the Show `key` names, then each of `episodes` in it.
    async fn match_episodes(
        &self,
        key: &TitleKey,
        episodes: &[PendingEpisode],
    ) -> Vec<(i64, Result<Metadata, Miss>)> {
        let show = match self.find(TitleKind::Show, key).await {
            Ok(id) => self
                .ask(|metadata| {
                    let id = id.clone();
                    async move { metadata.show(&id).await.map(|show| (id, show)) }
                })
                .await
                .map(|(plugin, (id, show))| (plugin, id, show)),
            Err(miss) => Err(miss),
        };
        let (plugin, show_id, show) = match show {
            Ok(found) => found,
            Err(miss) => return episodes.iter().map(|e| (e.item, Err(miss))).collect(),
        };
        let numbers: BTreeSet<i64> = episodes.iter().map(|e| e.season).collect();
        let mut seasons = BTreeMap::new();
        for number in numbers {
            seasons.insert(number, self.season(&show_id, number).await);
        }
        episodes
            .iter()
            .map(|pending| {
                let found = match seasons.get(&pending.season) {
                    Some(Ok(season)) => season
                        .episodes
                        .iter()
                        .find(|e| i64::from(e.number) == pending.episode)
                        .map(|episode| Metadata::episode(&plugin, &show, season, episode))
                        .ok_or(Miss::Unmatched),
                    Some(Err(miss)) => Err(*miss),
                    None => Err(Miss::Undecided),
                };
                (pending.item, found)
            })
            .collect()
    }

    /// The Season `number` of the Show `show`.
    async fn season(&self, show: &ExternalId, number: i64) -> Result<Season, Miss> {
        let number = u32::try_from(number).map_err(|_negative| Miss::Unmatched)?;
        self.ask(|metadata| {
            let show = show.clone();
            async move { metadata.season(&show, number).await }
        })
        .await
        .map(|(_, season)| season)
    }

    /// The id of the title `key` names: its ID hint, or else the search result that fits it.
    async fn find(&self, kind: TitleKind, key: &TitleKey) -> Result<ExternalId, Miss> {
        if let Some(id) = key.tmdb_id {
            return Ok(ExternalId::Tmdb(id));
        }
        let year = key.year.and_then(|year| i32::try_from(year).ok());
        let mut decision = self.search(kind, &key.title, year, year).await?;
        // A metadata source may only return titles from exactly the year searched for, so a year
        // off by one (release dates differ by country) finds nothing. Search again without it,
        // and let the confidence score weigh the year.
        if decision == Decision::NoMatch && year.is_some() {
            decision = self.search(kind, &key.title, None, year).await?;
        }
        match decision {
            Decision::Match(id) => Ok(id),
            Decision::Ambiguous => {
                tracing::info!(
                    title = %key.title,
                    year = ?key.year,
                    "more than one title fits; left Unmatched"
                );
                Err(Miss::Unmatched)
            }
            Decision::NoMatch => Err(Miss::Unmatched),
        }
    }

    /// Searches for `title` from `search_year`, and decides which result fits `title` from
    /// `year`.
    async fn search(
        &self,
        kind: TitleKind,
        title: &str,
        search_year: Option<i32>,
        year: Option<i32>,
    ) -> Result<Decision, Miss> {
        let query = SearchQuery {
            kind,
            title: title.to_owned(),
            year: search_year,
        };
        let (_, candidates) = self
            .ask(|metadata| {
                let query = query.clone();
                async move { metadata.search(&query).await }
            })
            .await?;
        Ok(confidence::decide(title, year, &candidates))
    }

    /// Asks the metadata Provider, and says which Plugin answered.
    async fn ask<R, F>(
        &self,
        call: impl Fn(Arc<dyn MetadataProvider>) -> F,
    ) -> Result<(PluginId, R), Miss>
    where
        F: Future<Output = Result<R, MetadataError>>,
    {
        match self
            .plugins
            .call_provider_with_id::<dyn MetadataProvider, _, _>(call)
            .await
        {
            Some((plugin, Ok(answer))) => Ok((plugin, answer)),
            Some((_, Err(MetadataError::NotFound | MetadataError::UnsupportedId(_)))) => {
                Err(Miss::Unmatched)
            }
            Some((plugin, Err(MetadataError::Failed(error)))) => {
                tracing::warn!(%plugin, %error, "the metadata provider failed; will try again");
                Err(Miss::Undecided)
            }
            None => {
                tracing::debug!("no metadata provider is enabled; nothing can be Matched");
                Err(Miss::Undecided)
            }
        }
    }

    /// Stores what was decided about each item, counting it in `summary`.
    async fn store(
        &self,
        outcomes: Vec<(i64, Result<Metadata, Miss>)>,
        summary: &mut Summary,
    ) -> Result<(), sqlx::Error> {
        let mut decided = Vec::new();
        for (item, outcome) in outcomes {
            match outcome {
                Ok(metadata) => {
                    summary.matched += 1;
                    decided.push((item, Decided::Matched(Box::new(metadata))));
                }
                Err(Miss::Unmatched) => {
                    summary.unmatched += 1;
                    decided.push((item, Decided::Unmatched));
                }
                Err(Miss::Undecided) => summary.undecided += 1,
            }
        }
        if decided.is_empty() {
            return Ok(());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
            });
        self.db
            .write(move |tx| Box::pin(insert_decisions(tx, decided, now)))
            .await
    }
}

/// `n` items, as a [`Summary`] counts them.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Stores `decided` as of `now`, for the items that still exist.
async fn insert_decisions(
    conn: &mut SqliteConnection,
    decided: Vec<(i64, Decided)>,
    now: i64,
) -> Result<(), sqlx::Error> {
    for (item, decision) in decided {
        let row = MatchRow::from(&decision);
        sqlx::query!(
            "INSERT OR REPLACE INTO item_matches (
                 item_id, state, plugin, tmdb_id, imdb_id, tvdb_id,
                 show_tmdb_id, show_imdb_id, show_tvdb_id, title, original_title, show_title,
                 overview, release_date, runtime_minutes, poster, backdrop, decided_at)
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
             WHERE EXISTS (SELECT 1 FROM library_items WHERE id = ?)",
            item,
            row.state,
            row.plugin,
            row.ids.tmdb,
            row.ids.imdb,
            row.ids.tvdb,
            row.show_ids.tmdb,
            row.show_ids.imdb,
            row.show_ids.tvdb,
            row.metadata.title,
            row.metadata.original_title,
            row.metadata.show_title,
            row.metadata.overview,
            row.metadata.release_date,
            row.metadata.runtime_minutes,
            row.metadata.poster,
            row.metadata.backdrop,
            now,
            item,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Whether an item is Matched, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum MatchState {
    /// The item has a Match.
    Matched,
    /// No title fit the item confidently.
    Unmatched,
}

/// A decision as the columns of `item_matches`.
#[derive(Debug)]
struct MatchRow<'a> {
    state: MatchState,
    plugin: Option<&'a str>,
    ids: ExternalIds,
    show_ids: ExternalIds,
    metadata: MetadataColumns<'a>,
}

/// A Match's metadata as columns of `item_matches`; all empty for an Unmatched item.
#[derive(Debug, Default)]
struct MetadataColumns<'a> {
    title: Option<&'a str>,
    original_title: Option<&'a str>,
    show_title: Option<&'a str>,
    overview: Option<&'a str>,
    release_date: Option<&'a str>,
    runtime_minutes: Option<u32>,
    poster: Option<&'a str>,
    backdrop: Option<&'a str>,
}

impl<'a> From<&'a Decided> for MatchRow<'a> {
    fn from(decided: &'a Decided) -> Self {
        match decided {
            Decided::Unmatched => Self {
                state: MatchState::Unmatched,
                plugin: None,
                ids: ExternalIds::default(),
                show_ids: ExternalIds::default(),
                metadata: MetadataColumns::default(),
            },
            Decided::Matched(metadata) => Self {
                state: MatchState::Matched,
                plugin: Some(metadata.plugin.as_str()),
                ids: metadata.ids.clone(),
                show_ids: metadata.show_ids.clone(),
                metadata: MetadataColumns {
                    title: Some(&metadata.title),
                    original_title: metadata.original_title.as_deref(),
                    show_title: metadata.show_title.as_deref(),
                    overview: metadata.overview.as_deref(),
                    release_date: metadata.release_date.as_deref(),
                    runtime_minutes: metadata.runtime_minutes,
                    poster: metadata.poster.as_deref(),
                    backdrop: metadata.backdrop.as_deref(),
                },
            },
        }
    }
}

/// What Matching stored for an item, as tests read it back.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stored {
    /// The file's path.
    pub(crate) path: String,
    /// For an Episode, its Season and number.
    pub(crate) episode: Option<(i64, i64)>,
    /// `None` while undecided.
    pub(crate) state: Option<MatchState>,
    pub(crate) plugin: Option<String>,
    pub(crate) ids: ExternalIds,
    pub(crate) show_ids: ExternalIds,
    pub(crate) title: Option<String>,
    pub(crate) show_title: Option<String>,
    pub(crate) poster: Option<String>,
}

#[cfg(test)]
impl Matcher {
    /// Every item of the Library `library` and what Matching stored for it, by path.
    pub(crate) async fn stored(&self, library: i64) -> Vec<Stored> {
        sqlx::query!(
            r#"SELECT library_files.path, library_items.season, library_items.episode,
                      item_matches.state AS "state?: MatchState", item_matches.plugin,
                      item_matches.tmdb_id, item_matches.imdb_id, item_matches.tvdb_id,
                      item_matches.show_tmdb_id, item_matches.show_imdb_id,
                      item_matches.show_tvdb_id, item_matches.title, item_matches.show_title,
                      item_matches.poster
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               LEFT JOIN item_matches ON item_matches.item_id = library_items.id
               WHERE library_files.library_id = ?
               ORDER BY library_files.path, library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await
        .unwrap()
        .into_iter()
        .map(|row| Stored {
            path: row.path,
            episode: row.season.zip(row.episode),
            state: row.state,
            plugin: row.plugin,
            ids: ExternalIds {
                tmdb: row.tmdb_id,
                imdb: row.imdb_id,
                tvdb: row.tvdb_id,
            },
            show_ids: ExternalIds {
                tmdb: row.show_tmdb_id,
                imdb: row.show_imdb_id,
                tvdb: row.show_tvdb_id,
            },
            title: row.title,
            show_title: row.show_title,
            poster: row.poster,
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};

    use nama_plugin_api::{
        PluginId,
        metadata::ExternalIds,
        settings::{SettingValue, Settings},
    };
    use nama_plugin_tmdb::{
        Tmdb,
        testing::{API_KEY, FakeTmdb},
    };
    use tempfile::TempDir;

    use super::{MatchState, Matcher, Stored};
    use crate::{
        db::Database,
        libraries::{Kind, Libraries},
        plugins::Registry,
        scanner::Scanner,
    };

    /// A Movies Library and a Shows Library in scratch folders, a scanner, and a fake TMDB.
    struct Fixture {
        scanner: Scanner,
        matcher: Matcher,
        plugins: Registry,
        tmdb: FakeTmdb,
        movies: Library,
        shows: Library,
        _media: TempDir,
        _data: TempDir,
    }

    struct Library {
        id: i64,
        root: PathBuf,
    }

    impl Library {
        fn write(&self, relative: &str) {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"video").unwrap();
        }
    }

    impl Fixture {
        /// The fixture, with the TMDB Plugin enabled.
        async fn new() -> Self {
            let fixture = Self::without_tmdb().await;
            fixture.enable_tmdb();
            fixture
        }

        /// The fixture, with no metadata Provider enabled.
        async fn without_tmdb() -> Self {
            let data = tempfile::tempdir().unwrap();
            let media = tempfile::tempdir().unwrap();
            let db = Arc::new(Database::open(data.path()).await.unwrap());
            let libraries = Libraries::new(Arc::clone(&db));
            let create = async |name: &str, kind| {
                let root = media.path().canonicalize().unwrap().join(name);
                fs::create_dir(&root).unwrap();
                let library = libraries
                    .create(name, kind, vec![root.to_str().unwrap().to_owned()])
                    .await
                    .unwrap();
                Library {
                    id: library.id,
                    root,
                }
            };
            let movies = create("Movies", Kind::Movies).await;
            let shows = create("Shows", Kind::Shows).await;
            let plugins = Registry::new();
            Self {
                scanner: Scanner::new(Arc::clone(&db), plugins.clone()),
                matcher: Matcher::new(db, plugins.clone()),
                plugins,
                tmdb: FakeTmdb::start().await,
                movies,
                shows,
                _media: media,
                _data: data,
            }
        }

        fn enable_tmdb(&self) {
            let settings = Settings::new(BTreeMap::from([(
                "api_key".to_owned(),
                SettingValue::Text(API_KEY.to_owned()),
            )]));
            self.plugins
                .enable_with_settings(
                    Arc::new(Tmdb::with_endpoints(self.tmdb.endpoints())),
                    settings,
                )
                .unwrap();
        }

        async fn scan(&self, library: &Library) {
            self.scanner.scan(library.id).await.unwrap();
        }

        /// What was stored for each item of `library`, as (path relative to its folder, Season
        /// and Episode, what it Matched).
        async fn stored(&self, library: &Library) -> Vec<(String, Option<(i64, i64)>, Outcome)> {
            self.matcher
                .stored(library.id)
                .await
                .into_iter()
                .map(|stored| {
                    let relative = PathBuf::from(&stored.path)
                        .strip_prefix(&library.root)
                        .unwrap()
                        .components()
                        .map(|part| part.as_os_str().to_str().unwrap().to_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    (relative, stored.episode, Outcome::of(&stored))
                })
                .collect()
        }
    }

    /// What Matching decided about an item, in short.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Outcome {
        /// Matched to the title with this TMDB id and name.
        Matched(i64, String),
        Unmatched,
        Undecided,
    }

    impl Outcome {
        fn of(stored: &Stored) -> Self {
            match stored.state {
                Some(MatchState::Matched) => {
                    Self::Matched(stored.ids.tmdb.unwrap(), stored.title.clone().unwrap())
                }
                Some(MatchState::Unmatched) => Self::Unmatched,
                None => Self::Undecided,
            }
        }
    }

    fn matched(id: i64, title: &str) -> Outcome {
        Outcome::Matched(id, title.to_owned())
    }

    #[tokio::test]
    async fn a_scan_matches_movies_confidently_and_leaves_the_rest_unmatched() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        movies.write("Alien (1979)/Alien (1979).mkv");
        movies.write("Dune (2021)/Dune (2021).mkv");
        // An ID hint is used directly: there is no recorded search for Heat.
        movies.write("Heat (1995) {tmdb-949}/Heat (1995).mkv");
        // Two films called Hamlet came out in 2000.
        movies.write("Hamlet (2000)/Hamlet (2000).mkv");
        // TMDB finds nothing.
        movies.write("Home Movies (2019)/Home Movies (2019).mkv");
        // The ID hint names no title on TMDB.
        movies.write("Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv");
        // No title or year to search by: no item, so nothing to Match.
        movies.write("home video.mkv");

        media.scan(movies).await;

        assert_eq!(
            media.stored(movies).await,
            [
                (
                    "Alien (1979)/Alien (1979).mkv".to_owned(),
                    None,
                    matched(348, "Alien")
                ),
                (
                    "Dune (2021)/Dune (2021).mkv".to_owned(),
                    None,
                    matched(438_631, "Dune")
                ),
                (
                    "Hamlet (2000)/Hamlet (2000).mkv".to_owned(),
                    None,
                    Outcome::Unmatched
                ),
                (
                    "Heat (1995) {tmdb-949}/Heat (1995).mkv".to_owned(),
                    None,
                    matched(949, "Heat")
                ),
                (
                    "Home Movies (2019)/Home Movies (2019).mkv".to_owned(),
                    None,
                    Outcome::Unmatched
                ),
                (
                    "Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv".to_owned(),
                    None,
                    Outcome::Unmatched
                ),
            ]
        );
        let alien = media.matcher.stored(movies.id).await.remove(0);
        assert_eq!(alien.plugin.as_deref(), Some("tmdb"));
        assert_eq!(
            alien.ids,
            ExternalIds {
                tmdb: Some(348),
                imdb: Some("tt0078748".to_owned()),
                tvdb: None,
            }
        );
        assert_eq!(
            alien.poster.as_deref(),
            Some("/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
        );
    }

    #[tokio::test]
    async fn a_scan_matches_episodes_through_their_show_and_season() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        shows.write("Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv");
        shows.write("Dark (2017)/Season 01/Dark (2017) - S01E03.mkv");
        // Season 1 has no Episode 42.
        shows.write("Dark (2017)/Season 01/Dark (2017) - S01E42.mkv");
        // No year, and both the US and the UK Office fit.
        shows.write("The Office/Season 01/The Office - S01E01.mkv");

        media.scan(shows).await;

        let dark = "Dark (2017)/Season 01/Dark (2017)";
        assert_eq!(
            media.stored(shows).await,
            [
                (
                    format!("{dark} - S01E01-E02.mkv"),
                    Some((1, 1)),
                    matched(1_388_538, "Secrets")
                ),
                (
                    format!("{dark} - S01E01-E02.mkv"),
                    Some((1, 2)),
                    matched(1_388_539, "Lies")
                ),
                (
                    format!("{dark} - S01E03.mkv"),
                    Some((1, 3)),
                    matched(1_388_540, "Past and Present")
                ),
                (
                    format!("{dark} - S01E42.mkv"),
                    Some((1, 42)),
                    Outcome::Unmatched
                ),
                (
                    "The Office/Season 01/The Office - S01E01.mkv".to_owned(),
                    Some((1, 1)),
                    Outcome::Unmatched
                ),
            ]
        );
        let secrets = media.matcher.stored(shows.id).await.remove(0);
        assert_eq!(secrets.show_title.as_deref(), Some("Dark"));
        assert_eq!(
            secrets.show_ids,
            ExternalIds {
                tmdb: Some(70523),
                imdb: Some("tt5753856".to_owned()),
                tvdb: Some(334_824),
            }
        );
        // The Show and its Season were each fetched once.
        let requests = media.tmdb.requests();
        let count = |prefix: &str| requests.iter().filter(|r| r.starts_with(prefix)).count();
        assert_eq!(count("/3/tv/70523?"), 1, "{requests:?}");
        assert_eq!(count("/3/tv/70523/season/1?"), 1, "{requests:?}");
    }

    #[tokio::test]
    async fn a_year_off_by_one_still_matches_though_tmdb_filters_by_exact_year() {
        let media = Fixture::new().await;
        // Released in 1979 in the US, but 1980 where this copy came from. TMDB finds no Alien
        // from 1980, so the search is tried again without the year.
        media.movies.write("Alien (1980)/Alien (1980).mkv");

        media.scan(&media.movies).await;

        assert_eq!(
            media.stored(&media.movies).await[0].2,
            matched(348, "Alien")
        );
    }

    #[tokio::test]
    async fn a_failing_metadata_provider_stops_the_pass_and_leaves_items_undecided() {
        let media = Fixture::without_tmdb().await;
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text("rejected".to_owned()),
        )]));
        media
            .plugins
            .enable_with_settings(
                Arc::new(Tmdb::with_endpoints(media.tmdb.endpoints())),
                settings,
            )
            .unwrap();
        media.movies.write("Alien (1979)/Alien (1979).mkv");
        media.movies.write("Dune (2021)/Dune (2021).mkv");

        media.scan(&media.movies).await;

        // TMDB rejected the key once; nothing more was asked.
        assert_eq!(media.tmdb.requests().len(), 1);
        assert_eq!(
            media
                .stored(&media.movies)
                .await
                .into_iter()
                .map(|(_, _, outcome)| outcome)
                .collect::<Vec<_>>(),
            [Outcome::Undecided, Outcome::Undecided]
        );
    }

    #[tokio::test]
    async fn a_rescan_does_not_look_up_items_already_decided() {
        let media = Fixture::new().await;
        media.movies.write("Alien (1979)/Alien (1979).mkv");
        media.movies.write("Hamlet (2000)/Hamlet (2000).mkv");
        media.scan(&media.movies).await;
        let looked_up = media.tmdb.requests().len();

        media.scan(&media.movies).await;

        assert_eq!(media.tmdb.requests().len(), looked_up);
        assert_eq!(
            media
                .stored(&media.movies)
                .await
                .into_iter()
                .map(|(_, _, outcome)| outcome)
                .collect::<Vec<_>>(),
            [matched(348, "Alien"), Outcome::Unmatched]
        );
    }

    #[tokio::test]
    async fn items_stay_undecided_until_a_metadata_provider_can_match_them() {
        let media = Fixture::without_tmdb().await;
        media.movies.write("Alien (1979)/Alien (1979).mkv");

        media.scan(&media.movies).await;
        let before = media.stored(&media.movies).await;
        media.enable_tmdb();
        media.scan(&media.movies).await;

        assert_eq!(before[0].2, Outcome::Undecided);
        assert_eq!(
            media.stored(&media.movies).await[0].2,
            matched(348, "Alien")
        );
        assert!(
            media
                .plugins
                .enabled()
                .contains(&PluginId::from_static("tmdb"))
        );
    }
}
