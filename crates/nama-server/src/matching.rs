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
//! An admin can fix a Match by hand ([`Matcher::set_match`], [`Matcher::clear_match`]). That pins
//! the Match to what the item's file names, its title, year and ID hint, for every item in the
//! Library named the same way; for an Episode, that is every Episode of its Show. Matching follows
//! a pin instead of searching, so a pinned Match holds when a file changes or is renamed, and a
//! new Episode of a pinned Show Matches the pinned Show.
//!
//! A Matched or Unmatched item stays so on later scans until its file changes. An item Matching
//! couldn't decide, because no metadata Provider is enabled or the Provider failed (such as when
//! TMDB is unreachable or rejects the API key), has no Match and so is Unmatched too, but nothing
//! is stored for it: it is undecided, and the next scan tries it again. Once the Provider fails,
//! the rest of that pass is left for the next scan too.

mod confidence;

use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use nama_plugin_api::{
    PluginId,
    metadata::{
        Candidate, Episode, ExternalId, ExternalIds, MetadataError, MetadataProvider, Movie,
        SearchQuery, Season, Show, TitleKind,
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

/// Why a Match couldn't be searched for, set or cleared by hand.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ManualError {
    /// There is no item with this id.
    #[error("there is no item {0}")]
    NoSuchItem(i64),
    /// The metadata source has no title with this id, or can't look titles up by it.
    #[error("the metadata source has no title {0}")]
    NoSuchTitle(ExternalId),
    /// No metadata Provider is enabled, or it failed.
    #[error("no metadata provider could answer")]
    Unavailable,
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
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
    /// What an admin pinned for items named so.
    pin: Option<Pin>,
}

/// A Match an admin set or cleared by hand, which Matching follows instead of searching.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Pin {
    /// Match the Movie, or for Episodes the Show, with this id.
    Title(ExternalId),
    /// Leave the items Unmatched.
    Unmatched,
}

impl Pin {
    /// The pin stored in the `match_pins` row, if there is one, holding the id in `ids`.
    fn from_row(pinned: bool, ids: ExternalIds) -> Option<Self> {
        if !pinned {
            return None;
        }
        let id = ids
            .tmdb
            .map(ExternalId::Tmdb)
            .or_else(|| ids.imdb.map(ExternalId::Imdb))
            .or_else(|| ids.tvdb.map(ExternalId::Tvdb));
        Some(id.map_or(Self::Unmatched, Self::Title))
    }

    /// The pinned id, as the `pinned_*` columns of `match_pins` hold it.
    fn columns(&self) -> ExternalIds {
        let mut ids = ExternalIds::default();
        match self {
            Self::Title(ExternalId::Tmdb(id)) => ids.tmdb = Some(*id),
            Self::Title(ExternalId::Imdb(id)) => ids.imdb = Some(id.clone()),
            Self::Title(ExternalId::Tvdb(id)) => ids.tvdb = Some(*id),
            Self::Unmatched => {}
        }
        ids
    }
}

/// The items of a Library named like one item: copies of the same Movie, or the Episodes of the
/// same Show.
#[derive(Debug)]
struct Named {
    library: i64,
    kind: ItemKind,
    key: TitleKey,
    /// The Movies; empty for a Show.
    movies: Vec<i64>,
    /// The Episodes; empty for a Movie.
    episodes: Vec<PendingEpisode>,
}

impl Named {
    /// Every item's id.
    fn items(&self) -> impl Iterator<Item = i64> {
        let episodes = self.episodes.iter().map(|episode| episode.item);
        self.movies.iter().copied().chain(episodes)
    }
}

/// A Show the metadata Provider has, and the Plugin whose Provider it came from.
#[derive(Debug)]
struct FoundShow {
    plugin: PluginId,
    id: ExternalId,
    show: Show,
}

/// The items of a Library that Matching hasn't decided yet.
#[derive(Debug, Default)]
struct Pending {
    /// Movies, grouped by the title they name.
    movies: BTreeMap<TitleKey, Vec<i64>>,
    /// Episodes, grouped by the Show they're in.
    shows: BTreeMap<TitleKey, Vec<PendingEpisode>>,
    /// Episodes stored without a Season or Episode number, which no title can fit. The schema
    /// forbids them, so there should be none.
    unnumbered: Vec<i64>,
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

    /// The titles the metadata Provider finds for `query`, best fit first.
    pub(crate) async fn search_metadata(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<Candidate>, ManualError> {
        let found = self
            .ask(|metadata| {
                let query = query.clone();
                async move { metadata.search(&query).await }
            })
            .await;
        match found {
            Ok((_, candidates)) => Ok(candidates),
            Err(Miss::Unmatched) => Ok(Vec::new()),
            Err(Miss::Undecided) => Err(ManualError::Unavailable),
        }
    }

    /// The Library the item `item` is in.
    pub(crate) async fn library_of(&self, item: i64) -> Result<i64, ManualError> {
        sqlx::query_scalar!(
            "SELECT library_files.library_id
             FROM library_items
             JOIN library_files ON library_files.id = library_items.file_id
             WHERE library_items.id = ?",
            item
        )
        .fetch_optional(self.db.reader())
        .await?
        .ok_or(ManualError::NoSuchItem(item))
    }

    /// Matches the item `item` to the title `id` by hand, with every item named like it, and
    /// pins that Match. For an Episode, `id` names its Show, and each Episode of the Show is
    /// Matched through its Season, or Unmatched if the Show doesn't have it. Nothing changes
    /// unless every item could be decided. The caller holds the Library's scan lock.
    pub(crate) async fn set_match(
        &self,
        item: i64,
        id: &ExternalId,
    ) -> Result<Summary, ManualError> {
        let named = self.named_like(item).await?;
        let refusal = |miss| match miss {
            Miss::Unmatched => ManualError::NoSuchTitle(id.clone()),
            Miss::Undecided => ManualError::Unavailable,
        };
        let outcomes = match named.kind {
            ItemKind::Movie => {
                let movie = self.fetch_movie(id).await.map_err(refusal)?;
                named
                    .items()
                    .map(|alike| (alike, Ok(movie.clone())))
                    .collect()
            }
            ItemKind::Episode => {
                let show = self.fetch_show(id).await.map_err(refusal)?;
                self.match_in_show(&show, &named.episodes).await
            }
        };
        if outcomes
            .iter()
            .any(|(_, outcome)| matches!(outcome, Err(Miss::Undecided)))
        {
            return Err(ManualError::Unavailable);
        }
        self.pin(named, Pin::Title(id.clone()), outcomes).await
    }

    /// Removes the Match of the item `item` and of every item named like it, and pins them
    /// Unmatched. The caller holds the Library's scan lock.
    pub(crate) async fn clear_match(&self, item: i64) -> Result<Summary, ManualError> {
        let named = self.named_like(item).await?;
        let outcomes = named
            .items()
            .map(|alike| (alike, Err(Miss::Unmatched)))
            .collect();
        self.pin(named, Pin::Unmatched, outcomes).await
    }

    /// Matches the items of the Library `library` that Matching hasn't decided yet, until
    /// `cancel` is cancelled. The caller holds the Library's scan lock, so its items don't change
    /// meanwhile.
    pub(crate) async fn match_library(
        &self,
        library: i64,
        cancel: &CancellationToken,
    ) -> Result<Summary, Error> {
        let Pending {
            movies,
            shows,
            unnumbered,
        } = self.undecided(library).await?;
        let mut summary = Summary::default();
        if !unnumbered.is_empty() {
            tracing::error!(
                library,
                items = ?unnumbered,
                "Episodes without a Season or Episode number; left Unmatched"
            );
            let decided = unnumbered
                .into_iter()
                .map(|item| (item, Err(Miss::Unmatched)));
            self.store(decided.collect(), &mut summary).await?;
        }
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
    async fn undecided(&self, library: i64) -> Result<Pending, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_items.kind AS "kind: ItemKind",
                      library_items.title, library_items.year, library_items.tmdb_id,
                      library_items.season, library_items.episode,
                      match_pins.pinned_at IS NOT NULL AS "pinned!: bool",
                      match_pins.pinned_tmdb_id, match_pins.pinned_imdb_id,
                      match_pins.pinned_tvdb_id
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               LEFT JOIN match_pins ON match_pins.library_id = library_files.library_id
                                   AND match_pins.kind = library_items.kind
                                   AND match_pins.title = library_items.title
                                   AND match_pins.year IS library_items.year
                                   AND match_pins.tmdb_id IS library_items.tmdb_id
               WHERE library_files.library_id = ?
                 AND NOT EXISTS (SELECT 1 FROM item_matches
                                 WHERE item_matches.item_id = library_items.id)
               ORDER BY library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut pending = Pending::default();
        for row in rows {
            let pinned = ExternalIds {
                tmdb: row.pinned_tmdb_id,
                imdb: row.pinned_imdb_id,
                tvdb: row.pinned_tvdb_id,
            };
            let key = TitleKey {
                title: row.title,
                year: row.year,
                tmdb_id: row.tmdb_id,
                pin: Pin::from_row(row.pinned, pinned),
            };
            match (row.kind, row.season, row.episode) {
                (ItemKind::Movie, _, _) => pending.movies.entry(key).or_default().push(row.id),
                (ItemKind::Episode, Some(season), Some(episode)) => {
                    pending.shows.entry(key).or_default().push(PendingEpisode {
                        item: row.id,
                        season,
                        episode,
                    });
                }
                (ItemKind::Episode, _, _) => pending.unnumbered.push(row.id),
            }
        }
        Ok(pending)
    }

    /// The items of the item `item`'s Library named like it, with the pin they share, if any.
    async fn named_like(&self, item: i64) -> Result<Named, ManualError> {
        let rows = sqlx::query!(
            r#"SELECT files.library_id, named.id AS "id!", named.kind AS "kind: ItemKind",
                      named.title, named.year, named.tmdb_id, named.season, named.episode
               FROM library_items AS this
               JOIN library_files AS this_file ON this_file.id = this.file_id
               JOIN library_files AS files ON files.library_id = this_file.library_id
               JOIN library_items AS named ON named.file_id = files.id
               WHERE this.id = ?
                 AND named.kind = this.kind
                 AND named.title = this.title
                 AND named.year IS this.year
                 AND named.tmdb_id IS this.tmdb_id
               ORDER BY named.id"#,
            item
        )
        .fetch_all(self.db.reader())
        .await?;
        let first = rows.first().ok_or(ManualError::NoSuchItem(item))?;
        let mut named = Named {
            library: first.library_id,
            kind: first.kind,
            key: TitleKey {
                title: first.title.clone(),
                year: first.year,
                tmdb_id: first.tmdb_id,
                pin: None,
            },
            movies: Vec::new(),
            episodes: Vec::new(),
        };
        for row in rows {
            match (row.season, row.episode) {
                (Some(season), Some(episode)) => named.episodes.push(PendingEpisode {
                    item: row.id,
                    season,
                    episode,
                }),
                _ => named.movies.push(row.id),
            }
        }
        Ok(named)
    }

    /// Finds and fetches the Movie `key` names.
    async fn match_movie(&self, key: &TitleKey) -> Result<Metadata, Miss> {
        let id = self.find(TitleKind::Movie, key).await?;
        self.fetch_movie(&id).await
    }

    /// Fetches the Movie with the id `id`.
    async fn fetch_movie(&self, id: &ExternalId) -> Result<Metadata, Miss> {
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
            Ok(id) => self.fetch_show(&id).await,
            Err(miss) => Err(miss),
        };
        match show {
            Ok(show) => self.match_in_show(&show, episodes).await,
            Err(miss) => episodes.iter().map(|e| (e.item, Err(miss))).collect(),
        }
    }

    /// Fetches the Show with the id `id`.
    async fn fetch_show(&self, id: &ExternalId) -> Result<FoundShow, Miss> {
        let (plugin, show) = self
            .ask(|metadata| {
                let id = id.clone();
                async move { metadata.show(&id).await }
            })
            .await?;
        Ok(FoundShow {
            plugin,
            id: id.clone(),
            show,
        })
    }

    /// Fetches the Seasons `episodes` are in, from `show`, and finds each Episode in its Season.
    async fn match_in_show(
        &self,
        show: &FoundShow,
        episodes: &[PendingEpisode],
    ) -> Vec<(i64, Result<Metadata, Miss>)> {
        let FoundShow { plugin, id, show } = show;
        let mut by_season: BTreeMap<i64, Vec<&PendingEpisode>> = BTreeMap::new();
        for pending in episodes {
            by_season.entry(pending.season).or_default().push(pending);
        }
        let mut decided = Vec::with_capacity(episodes.len());
        for (number, in_season) in by_season {
            let season = self.season(id, number).await;
            decided.extend(in_season.into_iter().map(|pending| {
                let found = match &season {
                    Ok(season) => season
                        .episodes
                        .iter()
                        .find(|e| i64::from(e.number) == pending.episode)
                        .map(|episode| Metadata::episode(plugin, show, season, episode))
                        .ok_or(Miss::Unmatched),
                    Err(miss) => Err(*miss),
                };
                (pending.item, found)
            }));
        }
        decided
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

    /// The id of the title `key` names: what an admin pinned, its ID hint, or else the search
    /// result that fits it.
    async fn find(&self, kind: TitleKind, key: &TitleKey) -> Result<ExternalId, Miss> {
        match &key.pin {
            Some(Pin::Title(id)) => return Ok(id.clone()),
            Some(Pin::Unmatched) => return Err(Miss::Unmatched),
            None => {}
        }
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
        let decided = tally(outcomes, summary);
        if decided.is_empty() {
            return Ok(());
        }
        let now = now();
        self.db
            .write(move |tx| Box::pin(insert_decisions(tx, decided, now)))
            .await
    }

    /// Pins `pin` for the items `named` names, and stores what was decided about them, in one
    /// write.
    async fn pin(
        &self,
        named: Named,
        pin: Pin,
        outcomes: Vec<(i64, Result<Metadata, Miss>)>,
    ) -> Result<Summary, ManualError> {
        let mut summary = Summary::default();
        let decided = tally(outcomes, &mut summary);
        tracing::info!(
            library = named.library,
            title = %named.key.title,
            ?pin,
            ?summary,
            "fixing a Match by hand"
        );
        let now = now();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    insert_pin(tx, &named, &pin, now).await?;
                    insert_decisions(tx, decided, now).await
                })
            })
            .await?;
        Ok(summary)
    }
}

/// `n` items, as a [`Summary`] counts them.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The decisions among `outcomes`, each counted in `summary`.
fn tally(
    outcomes: Vec<(i64, Result<Metadata, Miss>)>,
    summary: &mut Summary,
) -> Vec<(i64, Decided)> {
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
    decided
}

/// Now, in seconds since the Unix epoch.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// Stores `pin` for the items `named` names, as of `now`, replacing any pin they had.
async fn insert_pin(
    conn: &mut SqliteConnection,
    named: &Named,
    pin: &Pin,
    now: i64,
) -> Result<(), sqlx::Error> {
    let pinned = pin.columns();
    sqlx::query!(
        "INSERT OR REPLACE INTO match_pins (
             library_id, kind, title, year, tmdb_id,
             pinned_tmdb_id, pinned_imdb_id, pinned_tvdb_id, pinned_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        named.library,
        named.kind,
        named.key.title,
        named.key.year,
        named.key.tmdb_id,
        pinned.tmdb,
        pinned.imdb,
        pinned.tvdb,
        now,
    )
    .execute(conn)
    .await?;
    Ok(())
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
    /// The item's id.
    pub(crate) item: i64,
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
            r#"SELECT library_items.id AS "item!", library_files.path, library_items.season,
                      library_items.episode,
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
            item: row.item,
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
        metadata::{ExternalId, ExternalIds},
        settings::{SettingValue, Settings},
    };
    use nama_plugin_tmdb::{
        Tmdb,
        testing::{API_KEY, FakeTmdb},
    };
    use tempfile::TempDir;

    use super::{ManualError, MatchState, Matcher, Stored};
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

        /// Replaces the file at `relative` with one of another size, as an upgrade would.
        fn replace(&self, relative: &str) {
            fs::write(self.root.join(relative), b"a better copy of the video").unwrap();
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
                    let relative = library.relative(&stored.path);
                    (relative, stored.episode, Outcome::of(&stored))
                })
                .collect()
        }

        /// The outcome for each item of `library`, in the order [`Self::stored`] gives them.
        async fn outcomes(&self, library: &Library) -> Vec<Outcome> {
            self.stored(library)
                .await
                .into_iter()
                .map(|(_, _, outcome)| outcome)
                .collect()
        }

        /// The id of the first item stored for the file at `relative` in `library`.
        async fn item(&self, library: &Library, relative: &str) -> i64 {
            self.matcher
                .stored(library.id)
                .await
                .into_iter()
                .find(|stored| library.relative(&stored.path) == relative)
                .unwrap()
                .item
        }

        /// How many searches TMDB was asked for.
        fn searches(&self) -> usize {
            self.tmdb
                .requests()
                .iter()
                .filter(|request| request.starts_with("/3/search/"))
                .count()
        }
    }

    impl Library {
        /// `path`, relative to the Library's folder, with `/` between its parts.
        fn relative(&self, path: &str) -> String {
            PathBuf::from(path)
                .strip_prefix(&self.root)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_str().unwrap().to_owned())
                .collect::<Vec<_>>()
                .join("/")
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

    #[tokio::test]
    async fn a_manual_match_survives_a_rescan_after_its_file_changes() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        // TMDB finds nothing for it.
        let home_movies = "Home Movies (2019)/Home Movies (2019).mkv";
        movies.write(home_movies);
        media.scan(movies).await;
        let unmatched = media.outcomes(movies).await;
        let item = media.item(movies, home_movies).await;

        media
            .matcher
            .set_match(item, &ExternalId::Tmdb(949))
            .await
            .unwrap();
        let searches = media.searches();
        movies.replace(home_movies);
        media.scan(movies).await;

        assert_eq!(unmatched, [Outcome::Unmatched]);
        assert_eq!(media.outcomes(movies).await, [matched(949, "Heat")]);
        // The rescan followed the pin rather than searching again.
        assert_eq!(media.searches(), searches);
    }

    #[tokio::test]
    async fn a_manual_match_replaces_an_automatic_one_for_every_file_named_alike() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        movies.write("Alien (1979)/Alien (1979).mkv");
        movies.write("Alien (1979)/Alien (1979) - Director's Cut.mkv");
        media.scan(movies).await;
        let item = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;

        let summary = media
            .matcher
            .set_match(item, &ExternalId::Tmdb(949))
            .await
            .unwrap();

        assert_eq!(summary.matched, 2);
        assert_eq!(
            media.outcomes(movies).await,
            [matched(949, "Heat"), matched(949, "Heat")]
        );
    }

    #[tokio::test]
    async fn re_matching_a_show_re_matches_its_episodes() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        // Both the US and the UK Office fit, so the scan leaves them Unmatched.
        shows.write("The Office/Season 01/The Office - S01E01.mkv");
        shows.write("The Office/Season 01/The Office - S01E02.mkv");
        media.scan(shows).await;
        let pilot = media
            .item(shows, "The Office/Season 01/The Office - S01E01.mkv")
            .await;

        media
            .matcher
            .set_match(pilot, &ExternalId::Tmdb(2316))
            .await
            .unwrap();
        let us = media.outcomes(shows).await;
        media
            .matcher
            .set_match(pilot, &ExternalId::Tmdb(2996))
            .await
            .unwrap();

        assert_eq!(
            us,
            [
                matched(2_316_001, "Pilot"),
                matched(2_316_002, "Diversity Day")
            ]
        );
        assert_eq!(
            media.outcomes(shows).await,
            [
                matched(2_996_001, "Downsize"),
                matched(2_996_002, "Work Experience")
            ]
        );
    }

    #[tokio::test]
    async fn a_new_episode_of_a_pinned_show_matches_the_pinned_show() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        shows.write("The Office/Season 01/The Office - S01E01.mkv");
        media.scan(shows).await;
        let pilot = media
            .item(shows, "The Office/Season 01/The Office - S01E01.mkv")
            .await;
        media
            .matcher
            .set_match(pilot, &ExternalId::Tmdb(2316))
            .await
            .unwrap();

        shows.write("The Office/Season 01/The Office - S01E02.mkv");
        media.scan(shows).await;

        assert_eq!(
            media.outcomes(shows).await[1],
            matched(2_316_002, "Diversity Day")
        );
    }

    #[tokio::test]
    async fn a_cleared_match_stays_unmatched_after_its_file_changes() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        let alien = "Alien (1979)/Alien (1979).mkv";
        movies.write(alien);
        media.scan(movies).await;
        let item = media.item(movies, alien).await;

        media.matcher.clear_match(item).await.unwrap();
        let cleared = media.outcomes(movies).await;
        movies.replace(alien);
        media.scan(movies).await;

        assert_eq!(cleared, [Outcome::Unmatched]);
        assert_eq!(media.outcomes(movies).await, [Outcome::Unmatched]);
    }

    #[tokio::test]
    async fn a_match_to_a_title_the_source_does_not_have_is_refused_and_changes_nothing() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        movies.write("Alien (1979)/Alien (1979).mkv");
        media.scan(movies).await;
        let item = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;

        let result = media.matcher.set_match(item, &ExternalId::Tmdb(1)).await;

        assert!(
            matches!(result, Err(ManualError::NoSuchTitle(ExternalId::Tmdb(1)))),
            "{result:?}"
        );
        assert_eq!(media.outcomes(movies).await, [matched(348, "Alien")]);
    }

    #[tokio::test]
    async fn a_match_for_an_item_that_does_not_exist_is_refused() {
        let media = Fixture::new().await;

        let result = media.matcher.set_match(42, &ExternalId::Tmdb(949)).await;

        assert!(
            matches!(result, Err(ManualError::NoSuchItem(42))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn a_match_set_while_no_metadata_provider_is_enabled_is_unavailable() {
        let media = Fixture::without_tmdb().await;
        media.movies.write("Alien (1979)/Alien (1979).mkv");
        media.scan(&media.movies).await;
        let item = media
            .item(&media.movies, "Alien (1979)/Alien (1979).mkv")
            .await;

        let result = media.matcher.set_match(item, &ExternalId::Tmdb(348)).await;

        assert!(
            matches!(result, Err(ManualError::Unavailable)),
            "{result:?}"
        );
    }
}
