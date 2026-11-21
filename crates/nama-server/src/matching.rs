//! Matching: linking the items a scan found to titles from a Library's title sources, and storing
//! those titles' metadata and ids on the items.
//!
//! After each scan, Matching decides about every item of the Library it hasn't decided yet, asking
//! the Library's title sources in its order (see [`sources`]):
//!
//! - An item whose name has an ID hint is fetched by that id directly, from the source keeping its
//!   namespace or else the first other to have it. If none has such a title, the item is
//!   Unmatched. A hint in a namespace no source looks titles up by, such as `{x264-GROUP}`, is
//!   ignored.
//! - Otherwise its title and year are searched for with the first source that finds any, and it
//!   Matches only a result that fits confidently and unambiguously (see [`confidence`]). Anything
//!   else is Unmatched, except that among Shows that tie, the one that alone has every Season and
//!   Episode its files name Matches. A Show's country tag, as in `The Office (UK)`, is left out of
//!   the search and keeps only Shows from that country.
//! - Before acting on what the search decided, it asks the enabled Recognisers (see
//!   [`recognisers`]) with the search's results and the item's paths. A title the first of them
//!   names by an id a source keeps or looks titles up by is fetched as an ID hint's is.
//! - A Match keeps every id the sources asked gave for its title, the answering source's winning.
//! - A Match stores the artwork Artwork picks from every ordered source's artwork list, each image
//!   with the Plugin that listed it, and the answering source's own poster or backdrop where the
//!   lists have none. An Episode takes its Season's own poster first.
//! - An Episode Matches once its Show does and the Show's Season, fetched from the source that gave
//!   the Show, lists it. Each Show is looked up once per pass, and each of its Seasons fetched
//!   once: every one but the Specials, even without an Episode here, for its air dates. An Episode
//!   keeps its own ids and its Show's; Watch State keys Episodes by the Show's ids plus Season and
//!   Episode numbers (ADR 0008).
//!
//! Matching stores why it left an item Unmatched, so an admin knows how to fix it: more than one
//! title fits about as well (and which), none does, its ID hint names none, or, for an Episode,
//! its Show has no such Season or Episode.
//!
//! An admin can fix a Match by hand ([`Matcher::set_match`], [`Matcher::clear_match`]). That pins
//! the Match to what the item's file names, its title, year and ID hint, for every item in the
//! Library named the same way; for an Episode, that is every Episode of its Show. Matching follows
//! a pin instead of searching, so a pinned Match holds when a file changes, or is renamed to a
//! name that still says the same title, year and ID hint, and a new Episode of a pinned Show
//! Matches the pinned Show. A rename that changes any of those is a new name, which isn't pinned.
//!
//! A Matched or Unmatched item stays so on later scans until its file changes, or, if it was left
//! Unmatched automatically, a newer name parser reads its file (ADR 0023). An item Matching
//! couldn't decide, because no title source is enabled or one failed (such as when TMDB is
//! unreachable or rejects the API key), or its pin's source isn't enabled, has no Match and so is
//! Unmatched too, but nothing is stored for it: it is undecided, and the next scan tries it again.
//! Once a source fails, the rest of that pass is left for the next scan too. A Recogniser that
//! fails leaves only the title it was asked about undecided.
//!
//! A Match stores what [`METADATA_VERSION`] says Matching takes from the title sources. After
//! deciding, each scan fetches the titles of Matches stored at an older version again, from the
//! Plugin that Matched them and by the ids they have, as a first Match does, and stores what it
//! gets (ADR 0040). It never searches, so each item keeps its title and pin. A title it couldn't
//! fetch keeps what it had, at its older version, and the next scan tries again.
//!
//! [`METADATA_VERSION`]: crate::library::METADATA_VERSION

mod confidence;
mod recognisers;
mod sources;
#[cfg(test)]
mod testing;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
    recogniser::RecogniserMethod,
};
use nama_proto::nama::v1::{Event, ItemMatched};
use tokio_util::sync::CancellationToken;

use crate::{
    db::{Database, detached},
    events::{EventBus, event},
    library::{
        Artwork, Contents, Decided, Decisions, ImageRef, Item, Libraries, LibraryError, Metadata,
        Name, Named, Picked, Pin, Recognisers, ScanLock, ScanLocks, Source, SourceEpisode,
        SourceSeason, SourceShow, TitleKind, TitleSources, Unanswered, UnmatchedReason,
    },
    plugins::ExtensionPoints,
};

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
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// The Server is shutting down.
    #[error("matching was cancelled")]
    Cancelled,
}

/// Why a Match couldn't be searched for, set or cleared by hand.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ManualError {
    #[error("there is no item {0}")]
    NoSuchItem(i64),
    /// The title source has no title with this id, or can't look titles up by it.
    #[error("the title source has no title {0}")]
    NoSuchTitle(ExternalId),
    /// No title source is enabled, or one failed.
    #[error("no title source could answer")]
    Unavailable,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub(crate) struct Matcher {
    contents: Contents,
    /// Reads a Library's title source order.
    libraries: Libraries,
    metadata: TitleSources,
    recognisers: Recognisers,
    /// Picks a Match's artwork, and downloads the images of the Matches set by hand.
    artwork: Artwork,
    events: EventBus,
    locks: ScanLocks,
}

/// The title an item names: a Movie, or the Show an Episode is in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TitleKey {
    name: Name,
    pin: Option<Pin>,
}

/// A Show, the source that gave it, and its id there, which its Seasons are fetched by.
#[derive(Debug)]
struct FoundShow {
    source: Source,
    id: ExternalId,
    show: SourceShow,
}

#[derive(Debug, Default)]
struct Pending {
    movies: BTreeMap<TitleKey, Vec<i64>>,
    shows: BTreeMap<TitleKey, Vec<PendingEpisode>>,
    /// The paths of each title's item files, relative to the Library's folder, for the Recognisers.
    paths: BTreeMap<TitleKey, BTreeSet<String>>,
}

/// The Matches stored at an older [`METADATA_VERSION`], by the Plugin that Matched them and the id
/// of their title: a Movie, or the Show an Episode is in.
///
/// [`METADATA_VERSION`]: crate::library::METADATA_VERSION
#[derive(Debug, Default)]
struct Outdated {
    movies: BTreeMap<(PluginId, ExternalId), Vec<i64>>,
    shows: BTreeMap<(PluginId, ExternalId), Vec<PendingEpisode>>,
}

/// What looking items up found for each, with the Seasons fetched with their Show, by the Plugin
/// that gave them and every id of their Show, which Season Requests and air dates follow.
#[derive(Debug, Default)]
struct Outcomes {
    items: Vec<(i64, Result<Metadata, Miss>)>,
    seasons: Vec<(PluginId, ExternalIds, SourceSeason)>,
}

impl Outcomes {
    fn any_undecided(&self) -> bool {
        self.items
            .iter()
            .any(|(_, outcome)| matches!(outcome, Err(Miss::Undecided)))
    }
}

impl FromIterator<(i64, Result<Metadata, Miss>)> for Outcomes {
    fn from_iter<I: IntoIterator<Item = (i64, Result<Metadata, Miss>)>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().collect(),
            seasons: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PendingEpisode {
    item: i64,
    season: i64,
    episode: i64,
}

impl PendingEpisode {
    const fn of(item: i64, what: &Item) -> Option<Self> {
        match *what {
            Item::Episode {
                season, episode, ..
            } => Some(Self {
                item,
                season,
                episode,
            }),
            Item::Movie { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Miss {
    /// With why, or `None` where Matching stores none: a pin, or a title a source can't find
    /// that no ID Hint named.
    Unmatched(Option<UnmatchedReason>),
    /// The title sources couldn't answer: the item is Unmatched for now, but undecided, so
    /// the next scan tries again.
    Undecided,
    /// A Recogniser failed for this title: undecided like [`Miss::Undecided`], but the title
    /// sources still answer, so the sources go on Matching the Library's other titles.
    RecogniserFailed,
    /// The item's pin names a title no source the Library asks looks titles up by: undecided until
    /// one does, while the sources go on Matching the Library's other titles.
    Unserved,
}

impl Miss {
    const NOT_IN_SHOW: Self = Self::Unmatched(Some(UnmatchedReason::NotInShow));
}

impl From<Unanswered> for Miss {
    fn from(err: Unanswered) -> Self {
        match err {
            Unanswered::NotFound => Self::Unmatched(None),
            Unanswered::Unavailable => Self::Undecided,
        }
    }
}

impl ManualError {
    /// Why looking up the title `id` by hand failed.
    fn looking_up(id: &ExternalId) -> impl FnOnce(Unanswered) -> Self {
        |err| match err {
            Unanswered::NotFound => Self::NoSuchTitle(id.clone()),
            Unanswered::Unavailable => Self::Unavailable,
        }
    }
}

impl Metadata {
    /// This title with the artwork `picked`, keeping its own poster or backdrop where none was.
    fn with_picked(self, picked: &Picked) -> Self {
        let picked = picked.clone();
        Self {
            poster: picked.poster.or(self.poster),
            backdrop: picked.backdrop.or(self.backdrop),
            titled_backdrop: picked.titled_backdrop,
            logo: picked.logo.or(self.logo),
            ..self
        }
    }

    /// An Episode of `show`, with its Season's own poster, else the Show's `picked` artwork.
    fn episode(
        plugin: &PluginId,
        (show, picked): (&SourceShow, &Picked),
        season: &SourceSeason,
        episode: &SourceEpisode,
    ) -> Self {
        let poster = ImageRef::of(plugin, season.poster.as_ref());
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
            poster: (poster.or_else(|| picked.poster.clone()))
                .or_else(|| ImageRef::of(plugin, show.poster.as_ref())),
            backdrop: (picked.backdrop.clone())
                .or_else(|| ImageRef::of(plugin, show.backdrop.as_ref())),
            titled_backdrop: picked.titled_backdrop.clone(),
            logo: picked.logo.clone(),
            still: episode.still.clone(),
            genres: show.genres.clone(),
            content_rating: show.content_rating.clone(),
            show_poster: (picked.poster.clone())
                .or_else(|| ImageRef::of(plugin, show.poster.as_ref())),
            show_overview: show.overview.clone(),
            show_first_air_date: show.first_air_date.clone(),
            score: show.score,
            original_language: show.original_language.clone(),
            similar: show.similar.clone(),
            trailers: show.trailers.clone(),
        }
    }
}

impl Matcher {
    /// Shares `locks` with the Scanner, so a Match fixed by hand waits for a scan of its Library.
    pub(crate) fn new(
        db: Arc<Database>,
        plugins: ExtensionPoints,
        artwork: Artwork,
        events: EventBus,
        locks: ScanLocks,
    ) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            recognisers: Recognisers::new(plugins.clone(), RecogniserMethod::Title),
            contents: Contents::new(db),
            metadata: TitleSources::new(plugins),
            artwork,
            events,
            locks,
        }
    }

    /// Matches the item `item` to the title `id` by hand, with every item named like it, and
    /// pins that Match, after any scan of its Library. For an Episode, `id` names its Show, and
    /// each Episode of the Show is Matched through its Season, or Unmatched if the Show doesn't
    /// have it. Nothing changes unless every item could be decided. The new title's images are
    /// downloaded in the background, and the old one's deleted unless another Match stores them.
    pub(crate) async fn set_match(
        &self,
        item: i64,
        id: &ExternalId,
    ) -> Result<Summary, ManualError> {
        let (_scan, named) = self.lock_named_like(item).await?;
        let ordered = match self.libraries.get(named.library).await {
            Ok(found) => self.metadata.in_order(found.title_sources),
            Err(LibraryError::Database(err)) => return Err(err.into()),
            // The only other failure of a read is the Library being gone since its items were read.
            Err(_) => return Err(ManualError::NoSuchItem(item)),
        };
        // No enabled source keeps or looks up the namespace, so none could have the title; with
        // no source at all, fetching answers Unavailable instead.
        if self.metadata.unserved(id, named.name.kind().method()).await {
            return Err(ManualError::NoSuchTitle(id.clone()));
        }
        let outcomes = match named.name.kind() {
            TitleKind::Movie => {
                let movie = sources::fetch_movie(&self.metadata, id, ExternalIds::default())
                    .await
                    .map_err(ManualError::looking_up(id))?;
                let movie = self.picked_for(&ordered, movie).await;
                named
                    .items
                    .iter()
                    .map(|&(alike, _)| (alike, Ok(movie.clone())))
                    .collect()
            }
            TitleKind::Show => {
                let show = sources::fetch_show(&self.metadata, id, ExternalIds::default())
                    .await
                    .map_err(ManualError::looking_up(id))?;
                let episodes: Vec<PendingEpisode> = named
                    .items
                    .iter()
                    .filter_map(|(alike, what)| PendingEpisode::of(*alike, what))
                    .collect();
                let picked = self.pick_show(&ordered, &show).await;
                self.match_in_show(&show, &picked, &episodes).await
            }
        };
        if outcomes.any_undecided() {
            return Err(ManualError::Unavailable);
        }
        self.pin(item, named, Pin::Title(id.clone()), outcomes)
            .await
    }

    /// Removes the Match of the item `item` and of every item named like it, and pins them
    /// Unmatched, after any scan of its Library. Its images are deleted unless another Match
    /// stores them.
    pub(crate) async fn clear_match(&self, item: i64) -> Result<Summary, ManualError> {
        let (_scan, named) = self.lock_named_like(item).await?;
        let outcomes = named
            .items
            .iter()
            .map(|&(alike, _)| (alike, Err(Miss::Unmatched(None))))
            .collect();
        self.pin(item, named, Pin::Unmatched, outcomes).await
    }

    /// Matches the items of the Library `scan` covers that Matching hasn't decided yet, until
    /// `cancel` is cancelled.
    pub(crate) async fn match_library(
        &self,
        scan: &ScanLock,
        cancel: &CancellationToken,
    ) -> Result<Summary, Error> {
        let library = scan.library();
        let (this, folders) = match self.libraries.get(library).await {
            Ok(found) => (
                self.in_order(found.title_sources, &found.recognisers),
                found.folders,
            ),
            Err(LibraryError::Database(err)) => return Err(err.into()),
            // The only other failure of a read is the Library being gone, with nothing to Match.
            Err(_) => return Ok(Summary::default()),
        };
        let Pending {
            movies,
            shows,
            paths: mut paths_by_title,
        } = self.undecided(library, &folders).await?;
        let mut paths_of =
            |key: &TitleKey| Vec::from_iter(paths_by_title.remove(key).unwrap_or_default());
        let mut summary = Summary::default();
        // Once a title source fails (or there is none), asking about the other titles would only
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
            let paths = paths_of(&key);
            let recognisers = &this.recognisers;
            let outcome = match sources::movie(&this.metadata, recognisers, &key, &paths).await {
                Ok(movie) => Ok(self.picked_for(&this.metadata, movie).await),
                Err(miss) => Err(miss),
            };
            available = !matches!(outcome, Err(Miss::Undecided));
            let decided = items.into_iter().map(|item| (item, outcome.clone()));
            self.store(library, decided.collect(), &mut summary).await?;
        }
        for (key, episodes) in shows {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if !available {
                summary.undecided += count(episodes.len());
                continue;
            }
            let decided = this.match_episodes(&key, &episodes, &paths_of(&key)).await;
            available = !decided.any_undecided();
            self.store(library, decided, &mut summary).await?;
        }
        if available {
            available = self.refresh(library, &this.metadata, cancel).await?;
        }
        if summary != Summary::default() {
            tracing::info!(library, ?summary, "matched a Library");
        }
        if !available {
            tracing::warn!(
                library,
                "no title source could answer; will try again next scan"
            );
        }
        Ok(summary)
    }

    /// The undecided items of the Library `library`, whose folders are `folders`.
    async fn undecided(&self, library: i64, folders: &[String]) -> Result<Pending, sqlx::Error> {
        let mut pending = Pending::default();
        let mut paths = self.contents.paths_of_undecided(library).await?;
        for (id, item, pin) in self.contents.undecided(library).await? {
            let key = TitleKey {
                name: item.name(),
                pin,
            };
            let path = paths.remove(&id);
            if let Some(path) = path.and_then(|path| recognisers::relative(&path, folders)) {
                pending.paths.entry(key.clone()).or_default().insert(path);
            }
            match PendingEpisode::of(id, &item) {
                Some(episode) => pending.shows.entry(key).or_default().push(episode),
                None => pending.movies.entry(key).or_default().push(id),
            }
        }
        Ok(pending)
    }

    /// Fetches the titles of the Library `library`'s Matches stored at an older
    /// [`METADATA_VERSION`] again and stores what it gets, until `cancel` is cancelled. Returns
    /// whether every title source asked could answer. Each title's artwork is picked again from
    /// `ordered`, the Library's sources in its order, while the title comes only from its Plugin.
    ///
    /// [`METADATA_VERSION`]: crate::library::METADATA_VERSION
    async fn refresh(
        &self,
        library: i64,
        ordered: &TitleSources,
        cancel: &CancellationToken,
    ) -> Result<bool, Error> {
        let Outdated { movies, shows } = self.outdated(library).await?;
        // Once a Plugin fails (or is disabled or gone), asking it about its other titles would
        // only fail too, but another Plugin's Matches can still be fetched.
        let mut unavailable = BTreeSet::new();
        for ((plugin, id), items) in movies {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if unavailable.contains(&plugin) {
                continue;
            }
            let movie = match self.only(plugin.clone()).metadata.movie(&id).await {
                Ok(movie) => self.picked_for(ordered, movie).await,
                Err(Unanswered::NotFound) => continue,
                Err(Unanswered::Unavailable) => {
                    unavailable.insert(plugin);
                    continue;
                }
            };
            let found = items.into_iter().map(|item| (item, Ok(movie.clone())));
            self.store_refreshed(found.collect()).await?;
        }
        for ((plugin, id), episodes) in shows {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if unavailable.contains(&plugin) {
                continue;
            }
            let only = self.only(plugin.clone());
            let show = match sources::fetch_show(&only.metadata, &id, ExternalIds::default()).await
            {
                Ok(show) => show,
                Err(Unanswered::NotFound) => continue,
                Err(Unanswered::Unavailable) => {
                    unavailable.insert(plugin);
                    continue;
                }
            };
            let picked = self.pick_show(ordered, &show).await;
            let found = only.match_in_show(&show, &picked, &episodes).await;
            if found.any_undecided() {
                unavailable.insert(plugin);
            }
            self.store_refreshed(found).await?;
        }
        Ok(unavailable.is_empty())
    }

    async fn outdated(&self, library: i64) -> Result<Outdated, sqlx::Error> {
        let mut outdated = Outdated::default();
        for (id, item, plugin, title) in self.contents.outdated(library).await? {
            let key = (plugin, title);
            match PendingEpisode::of(id, &item) {
                Some(episode) => outdated.shows.entry(key).or_default().push(episode),
                None => outdated.movies.entry(key).or_default().push(id),
            }
        }
        Ok(outdated)
    }

    /// This Matcher, asking only the title sources whose namespace is in `order`, and the enabled
    /// Recognisers whose Plugin id is in `recognisers`, each in its order.
    fn in_order(&self, order: Vec<String>, recognisers: &[String]) -> Self {
        Self {
            metadata: self.metadata.in_order(order),
            recognisers: self.recognisers.in_order(recognisers),
            ..self.clone()
        }
    }

    /// `movie` with the artwork picked for it from `ordered`'s sources.
    async fn picked_for(&self, ordered: &TitleSources, movie: Metadata) -> Metadata {
        let picked = (self.artwork)
            .pick_for(ordered, TitleKind::Movie, &movie.ids)
            .await;
        movie.with_picked(&picked)
    }

    /// The artwork picked for the Show `found` from `ordered`'s sources.
    async fn pick_show(&self, ordered: &TitleSources, found: &FoundShow) -> Picked {
        (self.artwork)
            .pick_for(ordered, TitleKind::Show, &found.show.ids)
            .await
    }

    /// This Matcher, asking only the Plugin `plugin`'s title source.
    fn only(&self, plugin: PluginId) -> Self {
        Self {
            metadata: self.metadata.only(plugin),
            ..self.clone()
        }
    }

    /// Stores the Matches found again in `found`. An item missed keeps the Match it has, since a
    /// refresh never changes which title an item Matches.
    async fn store_refreshed(&self, found: Outcomes) -> Result<(), sqlx::Error> {
        let items = (found.items.into_iter())
            .filter_map(|(item, outcome)| Some((item, Decided::Matched(Box::new(outcome.ok()?)))))
            .collect();
        let decided = Decisions {
            items,
            seasons: found.seasons,
        };
        self.events.publish(self.contents.record(decided).await?);
        Ok(())
    }

    /// Takes the scan lock of the Library the item `item` is in, then reads every item named
    /// like it, which no scan changes while the lock is held.
    async fn lock_named_like(&self, item: i64) -> Result<(ScanLock, Named), ManualError> {
        let library = self
            .contents
            .library_of(item)
            .await?
            .ok_or(ManualError::NoSuchItem(item))?;
        let scan = self.locks.lock(library).await;
        let named = self
            .contents
            .named_like(item)
            .await?
            .ok_or(ManualError::NoSuchItem(item))?;
        Ok((scan, named))
    }

    /// Matches `episodes`, their files at `paths`, in the Show `key` names.
    async fn match_episodes(
        &self,
        key: &TitleKey,
        episodes: &[PendingEpisode],
        paths: &[String],
    ) -> Outcomes {
        match sources::show(&self.metadata, &self.recognisers, key, paths, episodes).await {
            Ok(show) => {
                let picked = self.pick_show(&self.metadata, &show).await;
                self.match_in_show(&show, &picked, episodes).await
            }
            Err(miss) => episodes
                .iter()
                .map(|e| (e.item, Err(miss.clone())))
                .collect(),
        }
    }

    /// Matches `episodes` in the Show `found`, whose artwork is `picked`.
    async fn match_in_show(
        &self,
        found: &FoundShow,
        picked: &Picked,
        episodes: &[PendingEpisode],
    ) -> Outcomes {
        let FoundShow { source, id, show } = found;
        let mut by_season: BTreeMap<i64, Vec<&PendingEpisode>> = BTreeMap::new();
        // Every Season but the Specials is fetched, for how many of the Show's Episodes aired.
        for &number in show.seasons.iter().filter(|&&number| number > 0) {
            by_season.entry(number.into()).or_default();
        }
        for pending in episodes {
            by_season.entry(pending.season).or_default().push(pending);
        }
        let mut outcomes = Outcomes {
            items: Vec::with_capacity(episodes.len()),
            seasons: Vec::new(),
        };
        let in_show = |season: &SourceSeason, episode: &SourceEpisode| {
            Metadata::episode(&source.plugin, (show, picked), season, episode)
        };
        let show_ids = (Some(show.ids.clone()).filter(|ids| !ids.is_empty()))
            .unwrap_or_else(|| ExternalIds::from_iter([id.clone()]));
        for (number, in_season) in by_season {
            let season = sources::season(&self.metadata, found, number).await;
            outcomes.items.extend(in_season.into_iter().map(|pending| {
                let outcome = match &season {
                    Ok(season) => season
                        .episodes
                        .iter()
                        .find(|e| i64::from(e.number) == pending.episode)
                        .map(|episode| in_show(season, episode))
                        .ok_or(Miss::NOT_IN_SHOW),
                    Err(miss) => Err(miss.clone()),
                };
                (pending.item, outcome)
            }));
            if let Ok(season) = season {
                (outcomes.seasons).push((source.plugin.clone(), show_ids.clone(), season));
            }
        }
        outcomes
    }

    async fn store(
        &self,
        library: i64,
        outcomes: Outcomes,
        summary: &mut Summary,
    ) -> Result<(), sqlx::Error> {
        let decided = tally(outcomes, summary);
        if decided.items.is_empty() {
            return Ok(());
        }
        let mut happened = matched_events(library, &decided.items);
        happened.extend(self.contents.record(decided).await?);
        self.events.publish(happened);
        Ok(())
    }

    /// Stores the pin and the decisions in one write.
    async fn pin(
        &self,
        item: i64,
        named: Named,
        pin: Pin,
        outcomes: Outcomes,
    ) -> Result<Summary, ManualError> {
        let mut summary = Summary::default();
        let decided = tally(outcomes, &mut summary);
        tracing::info!(
            library = named.library,
            title = %named.name.title,
            ?pin,
            ?summary,
            "fixing a Match by hand"
        );
        let mut happened = matched_events(named.library, &decided.items);
        let this = self.clone();
        // The write commits even if the caller stops waiting, so what follows it must too (ADR 0029).
        detached(async move {
            let library = named.library;
            // The Library may have been deleted since its items were read.
            let made_available = this
                .contents
                .pin(named.library, named.name, pin, decided)
                .await?
                .ok_or(ManualError::NoSuchItem(item))?;
            happened.extend(made_available);
            this.events.publish(happened);
            this.artwork.drop_unused(&this.contents).await;
            this.artwork
                .download_missing(this.contents.clone(), library);
            Ok::<_, ManualError>(())
        })
        .await?;
        Ok(summary)
    }
}

pub(crate) fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// What `outcomes` decided, counted into `summary`. An item left undecided is left out.
fn tally(outcomes: Outcomes, summary: &mut Summary) -> Decisions {
    let mut decided = Vec::new();
    for (item, outcome) in outcomes.items {
        match outcome {
            Ok(metadata) => {
                summary.matched += 1;
                decided.push((item, Decided::Matched(Box::new(metadata))));
            }
            Err(Miss::Unmatched(reason)) => {
                summary.unmatched += 1;
                decided.push((item, Decided::Unmatched(reason)));
            }
            Err(Miss::Undecided | Miss::RecogniserFailed | Miss::Unserved) => {
                summary.undecided += 1;
            }
        }
    }
    Decisions {
        items: decided,
        seasons: outcomes.seasons,
    }
}

fn matched_events(library: i64, decided: &[(i64, Decided)]) -> Vec<Event> {
    decided
        .iter()
        .filter_map(|(item, decision)| match decision {
            Decided::Matched(metadata) => Some(event(ItemMatched {
                library_id: library,
                item_id: *item,
                title: metadata.title.clone(),
                ..Default::default()
            })),
            Decided::Unmatched(_) => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "matching/matching_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "matching/refresh_tests.rs"]
mod refresh_tests;

#[cfg(test)]
#[path = "matching/manual_tests.rs"]
mod manual_tests;
