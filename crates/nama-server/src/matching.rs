//! Matching: linking the items a scan found to titles from the metadata Provider, and storing
//! those titles' metadata and external ids on the items.
//!
//! After each scan, Matching decides about every item of the Library it hasn't decided yet:
//!
//! - An item whose name has an ID hint is fetched by that id directly. If the metadata source
//!   has no such title, the item is Unmatched.
//! - Otherwise its title and year are searched for, and it Matches only a result that fits
//!   confidently and unambiguously (see [`confidence`]). Anything else is Unmatched. A Show's
//!   country tag, as in `The Office (UK)`, is left out of the search and keeps only Shows from
//!   that country.
//! - An Episode Matches once its Show does and the Show's Season lists it. Each Show is looked up
//!   once per pass, and each of its Seasons fetched once. An Episode keeps its own TMDB id and its
//!   Show's external ids; Watch State keys Episodes by the Show's ids plus Season and Episode
//!   numbers (ADR 0008), so its own IMDb and TVDB ids aren't fetched.
//!
//! Matching stores why it left an item Unmatched, so an admin knows how to fix it: more than one
//! title fits about as well (and which), none does, or its ID hint names none.
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
//! couldn't decide, because no metadata Provider is enabled or the Provider failed (such as when
//! TMDB is unreachable or rejects the API key), has no Match and so is Unmatched too, but nothing
//! is stored for it: it is undecided, and the next scan tries it again. Once the Provider fails,
//! the rest of that pass is left for the next scan too.
//!
//! A Match stores what [`METADATA_VERSION`] says Matching takes from the Provider. After deciding,
//! each scan fetches the titles of Matches stored at an older version again, from the Plugin that
//! Matched them and by the ids they have, as a first Match does, and stores what it gets (ADR
//! 0040). It never searches, so each item keeps its title and pin. A title it couldn't fetch keeps
//! what it had, at its older version, and the next scan tries again.

mod confidence;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use nama_plugin_api::{
    PluginId,
    metadata::{
        Candidate, Episode, ExternalId, ExternalIds, SearchQuery, Season, Show, TMDB, TitleKind,
    },
};
use nama_proto::nama::v1::{Event, ItemMatched};
use tokio_util::sync::CancellationToken;

use crate::{
    artwork::Artwork,
    db::{Database, detached},
    events::{EventBus, event},
    library::contents::{
        Contents, Decided, Decisions, Item, Metadata, Name, Named, Pin, TiedTitle, UnmatchedReason,
    },
    metadata::{self, MetadataSource},
    plugins::ExtensionPoints,
    scanner::{ScanLock, ScanLocks, country_tag},
};

use confidence::Decision;

/// The version of what a Match stores from the metadata Provider, stored with each Match. See the
/// crate's `AGENTS.md` for when to bump it.
pub(crate) const METADATA_VERSION: i64 = 1;

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
    /// The metadata source has no title with this id, or can't look titles up by it.
    #[error("the metadata source has no title {0}")]
    NoSuchTitle(ExternalId),
    /// No metadata Provider is enabled, or it failed.
    #[error("no metadata Provider could answer")]
    Unavailable,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub(crate) struct Matcher {
    contents: Contents,
    metadata: MetadataSource,
    /// Downloads the images of the Matches set by hand.
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

impl TitleKey {
    /// `miss`, from fetching the title this key names, blamed on its ID Hint if that named it.
    fn blame_id_hint(&self, miss: Miss) -> Miss {
        match miss {
            Miss::Unmatched(None) if self.pin.is_none() && self.name.hint.is_some() => {
                Miss::Unmatched(Some(UnmatchedReason::UnknownIdHint))
            }
            miss @ (Miss::Unmatched(_) | Miss::Undecided) => miss,
        }
    }
}

#[derive(Debug)]
struct FoundShow {
    plugin: PluginId,
    id: ExternalId,
    show: Show,
}

#[derive(Debug, Default)]
struct Pending {
    movies: BTreeMap<TitleKey, Vec<i64>>,
    shows: BTreeMap<TitleKey, Vec<PendingEpisode>>,
}

/// The Matches stored at an older [`METADATA_VERSION`], by the Plugin that Matched them and the id
/// of their title: a Movie, or the Show an Episode is in.
#[derive(Debug, Default)]
struct Outdated {
    movies: BTreeMap<(PluginId, ExternalId), Vec<i64>>,
    shows: BTreeMap<(PluginId, ExternalId), Vec<PendingEpisode>>,
}

/// What looking items up found for each, with the Seasons fetched to find Episodes in, by every
/// id of their Show, which Season Requests follow.
#[derive(Debug, Default)]
struct Outcomes {
    items: Vec<(i64, Result<Metadata, Miss>)>,
    seasons: Vec<(ExternalIds, Season)>,
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
    /// With why, or `None` where Matching stores none: a pin, or an Episode its Show doesn't have.
    Unmatched(Option<UnmatchedReason>),
    /// The metadata Provider couldn't answer: the item is Unmatched for now, but undecided, so
    /// the next scan tries again.
    Undecided,
}

impl From<metadata::Error> for Miss {
    fn from(err: metadata::Error) -> Self {
        match err {
            metadata::Error::NotFound => Self::Unmatched(None),
            metadata::Error::Unavailable => Self::Undecided,
        }
    }
}

impl ManualError {
    /// Why looking up the title `id` by hand failed.
    fn looking_up(id: &ExternalId) -> impl FnOnce(metadata::Error) -> Self {
        |err| match err {
            metadata::Error::NotFound => Self::NoSuchTitle(id.clone()),
            metadata::Error::Unavailable => Self::Unavailable,
        }
    }
}

impl Metadata {
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
            still: episode.still.clone(),
        }
    }
}

impl Matcher {
    /// Shares `locks` with the Scanner, so a Match fixed by hand waits for a scan of its Library.
    pub(crate) const fn new(
        db: Arc<Database>,
        plugins: ExtensionPoints,
        artwork: Artwork,
        events: EventBus,
        locks: ScanLocks,
    ) -> Self {
        Self {
            contents: Contents::new(db),
            metadata: MetadataSource::new(plugins),
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
        let outcomes = match named.name.kind() {
            TitleKind::Movie => {
                let movie = self
                    .metadata
                    .movie(id)
                    .await
                    .map_err(ManualError::looking_up(id))?;
                named
                    .items
                    .iter()
                    .map(|&(alike, _)| (alike, Ok(movie.clone())))
                    .collect()
            }
            TitleKind::Show => {
                let show = self
                    .fetch_show(id)
                    .await
                    .map_err(ManualError::looking_up(id))?;
                let episodes: Vec<PendingEpisode> = named
                    .items
                    .iter()
                    .filter_map(|(alike, what)| PendingEpisode::of(*alike, what))
                    .collect();
                self.match_in_show(&show, &episodes).await
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
        let Pending { movies, shows } = self.undecided(library).await?;
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
            let decided = self.match_episodes(&key, &episodes).await;
            available = !decided.any_undecided();
            self.store(library, decided, &mut summary).await?;
        }
        if available {
            available = self.refresh(library, cancel).await?;
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

    async fn undecided(&self, library: i64) -> Result<Pending, sqlx::Error> {
        let mut pending = Pending::default();
        for (id, item, pin) in self.contents.undecided(library).await? {
            let key = TitleKey {
                name: item.name(),
                pin,
            };
            match PendingEpisode::of(id, &item) {
                Some(episode) => pending.shows.entry(key).or_default().push(episode),
                None => pending.movies.entry(key).or_default().push(id),
            }
        }
        Ok(pending)
    }

    /// Fetches the titles of the Library `library`'s Matches stored at an older
    /// [`METADATA_VERSION`] again and stores what it gets, until `cancel` is cancelled. Returns
    /// whether every Provider asked could answer.
    async fn refresh(&self, library: i64, cancel: &CancellationToken) -> Result<bool, Error> {
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
                Ok(movie) => movie,
                Err(metadata::Error::NotFound) => continue,
                Err(metadata::Error::Unavailable) => {
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
            let show = match only.fetch_show(&id).await {
                Ok(show) => show,
                Err(metadata::Error::NotFound) => continue,
                Err(metadata::Error::Unavailable) => {
                    unavailable.insert(plugin);
                    continue;
                }
            };
            let found = only.match_in_show(&show, &episodes).await;
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

    /// This Matcher, asking only the Plugin `plugin`'s metadata Provider.
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

    async fn match_movie(&self, key: &TitleKey) -> Result<Metadata, Miss> {
        let id = self.find(TitleKind::Movie, key).await?;
        self.metadata
            .movie(&id)
            .await
            .map_err(|err| key.blame_id_hint(err.into()))
    }

    async fn match_episodes(&self, key: &TitleKey, episodes: &[PendingEpisode]) -> Outcomes {
        let show = match self.find(TitleKind::Show, key).await {
            Ok(id) => self
                .fetch_show(&id)
                .await
                .map_err(|err| key.blame_id_hint(err.into())),
            Err(miss) => Err(miss),
        };
        match show {
            Ok(show) => self.match_in_show(&show, episodes).await,
            Err(miss) => episodes
                .iter()
                .map(|e| (e.item, Err(miss.clone())))
                .collect(),
        }
    }

    async fn fetch_show(&self, id: &ExternalId) -> Result<FoundShow, metadata::Error> {
        let (plugin, show) = self.metadata.show(id).await?;
        // Seasons are fetched by the Show's TMDB id, which an ID Hint from TVDB or IMDb isn't.
        let id = show.ids.get(TMDB).unwrap_or_else(|| id.clone());
        Ok(FoundShow { plugin, id, show })
    }

    async fn match_in_show(&self, show: &FoundShow, episodes: &[PendingEpisode]) -> Outcomes {
        let FoundShow { plugin, id, show } = show;
        let mut by_season: BTreeMap<i64, Vec<&PendingEpisode>> = BTreeMap::new();
        for pending in episodes {
            by_season.entry(pending.season).or_default().push(pending);
        }
        let mut outcomes = Outcomes {
            items: Vec::with_capacity(episodes.len()),
            seasons: Vec::new(),
        };
        for (number, in_season) in by_season {
            let season = self.season(id, number).await;
            outcomes.items.extend(in_season.into_iter().map(|pending| {
                let found = match &season {
                    Ok(season) => season
                        .episodes
                        .iter()
                        .find(|e| i64::from(e.number) == pending.episode)
                        .map(|episode| Metadata::episode(plugin, show, season, episode))
                        .ok_or(Miss::Unmatched(None)),
                    Err(miss) => Err(miss.clone()),
                };
                (pending.item, found)
            }));
            if let Ok(season) = season {
                let show_ids = (Some(show.ids.clone()).filter(|ids| !ids.is_empty()))
                    .unwrap_or_else(|| ExternalIds::from_iter([id.clone()]));
                outcomes.seasons.push((show_ids, season));
            }
        }
        outcomes
    }

    async fn season(&self, show: &ExternalId, number: i64) -> Result<Season, Miss> {
        let number = u32::try_from(number).map_err(|_negative| Miss::Unmatched(None))?;
        Ok(self.metadata.season(show, number).await?)
    }

    /// The id of the title `key` names: what an admin pinned, its ID hint, or else the search
    /// result that fits it.
    async fn find(&self, kind: TitleKind, key: &TitleKey) -> Result<ExternalId, Miss> {
        match &key.pin {
            Some(Pin::Title(id)) => return Ok(id.clone()),
            Some(Pin::Unmatched) => return Err(Miss::Unmatched(None)),
            None => {}
        }
        let name = &key.name;
        if let Some(id) = &name.hint {
            return Ok(id.clone());
        }
        let year = name.year.and_then(|year| i32::try_from(year).ok());
        let (title, country) = match (kind, country_tag(&name.title)) {
            (TitleKind::Show, Some((title, country))) => (title, Some(country)),
            _ => (name.title.as_str(), None),
        };
        let mut query = SearchQuery {
            kind,
            title: title.to_owned(),
            year,
        };
        let mut decision = confidence::decide(title, year, &self.search(&query, country).await?);
        // A metadata source may only return titles from exactly the year searched for, so a year
        // off by one (release dates differ by country) finds nothing. Search again without it,
        // and let the confidence score weigh the year.
        if decision == Decision::NoMatch && year.is_some() {
            query.year = None;
            decision = confidence::decide(title, year, &self.search(&query, country).await?);
        }
        match decision {
            Decision::Match(id) => Ok(id),
            Decision::Ambiguous(tied) => {
                tracing::info!(
                    title = %name.title,
                    year = ?name.year,
                    "more than one title fits; left Unmatched"
                );
                let tied = tied.into_iter().map(TiedTitle::from).collect();
                Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(tied))))
            }
            Decision::NoMatch => Err(Miss::Unmatched(Some(UnmatchedReason::NoTitleFits))),
        }
    }

    /// What the Provider finds for `query`, keeping only titles from `country` if given.
    async fn search(
        &self,
        query: &SearchQuery,
        country: Option<&str>,
    ) -> Result<Vec<Candidate>, Miss> {
        let (_, mut candidates) = self.metadata.search(query).await?;
        if let Some(country) = country {
            candidates.retain(|candidate| candidate.origin_country.iter().any(|c| c == country));
        }
        Ok(candidates)
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
            Err(Miss::Undecided) => summary.undecided += 1,
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
#[path = "matching_tests.rs"]
mod tests;
