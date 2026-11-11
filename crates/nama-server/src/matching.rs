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

mod confidence;

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{Candidate, Episode, ExternalId, SearchQuery, Season, Show, TitleKind},
};
use nama_proto::nama::v1::{Event, ItemMatched};
use tokio_util::sync::CancellationToken;

use crate::{
    contents::{
        Contents, Decided, Decisions, Item, Metadata, Name, Named, Pin, TiedTitle, UnmatchedReason,
    },
    db::{Database, detached},
    events::{EventBus, event},
    metadata::{self, MetadataSource},
    plugins::Registry,
    scanner::{ScanLock, country_tag},
};

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
    events: EventBus,
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

/// What looking items up found for each, with the Seasons fetched to find Episodes in, by their
/// Show's TMDB id, which Season Requests follow.
#[derive(Debug, Default)]
struct Outcomes {
    items: Vec<(i64, Result<Metadata, Miss>)>,
    seasons: Vec<(i64, Season)>,
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
        }
    }
}

impl Matcher {
    pub(crate) const fn new(db: Arc<Database>, plugins: Registry, events: EventBus) -> Self {
        Self {
            contents: Contents::new(db),
            metadata: MetadataSource::new(plugins),
            events,
        }
    }

    /// Matches the item `item` to the title `id` by hand, with every item named like it, and
    /// pins that Match. For an Episode, `id` names its Show, and each Episode of the Show is
    /// Matched through its Season, or Unmatched if the Show doesn't have it. Nothing changes
    /// unless every item could be decided.
    pub(crate) async fn set_match(
        &self,
        scan: &ScanLock,
        item: i64,
        id: &ExternalId,
    ) -> Result<Summary, ManualError> {
        let named = self.named_like(scan, item).await?;
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
    /// Unmatched.
    pub(crate) async fn clear_match(
        &self,
        scan: &ScanLock,
        item: i64,
    ) -> Result<Summary, ManualError> {
        let named = self.named_like(scan, item).await?;
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

    async fn named_like(&self, scan: &ScanLock, item: i64) -> Result<Named, ManualError> {
        let named = self
            .contents
            .named_like(item)
            .await?
            .ok_or(ManualError::NoSuchItem(item))?;
        debug_assert_eq!(
            named.library,
            scan.library(),
            "item {item} is fixed under another Library's scan lock"
        );
        Ok(named)
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
        let id = show.ids.tmdb.map_or_else(|| id.clone(), ExternalId::Tmdb);
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
            if let (Ok(season), Some(show_tmdb_id)) = (season, show.ids.tmdb) {
                outcomes.seasons.push((show_tmdb_id, season));
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
        // The write commits even if the caller stops waiting, so its events must follow (ADR 0029).
        detached(async move {
            // The Library may have been deleted since its items were read.
            let made_available = this
                .contents
                .pin(named.library, named.name, pin, decided)
                .await?
                .ok_or(ManualError::NoSuchItem(item))?;
            happened.extend(made_available);
            this.events.publish(happened);
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
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};

    use futures_util::{FutureExt, StreamExt};
    use nama_plugin_api::{
        PluginId,
        metadata::{ExternalId, ExternalIds},
        settings::{SettingValue, Settings},
    };
    use nama_plugin_tmdb::{
        Tmdb,
        testing::{API_KEY, FakeTmdb},
    };
    use nama_proto::nama::v1::{EventType, ItemMatched, subscribe_response};
    use tempfile::TempDir;

    use super::{ManualError, Matcher, Summary};
    use crate::{
        contents::{Change, Contents, Decided, FileItem, Item, Recorded},
        db::Database,
        events::{EventBus, Filter, event},
        libraries::{Kind, Libraries},
        plugins::Registry,
        scanner::Scanner,
        server_settings::ServerUrl,
    };

    /// A Movies Library and a Shows Library in scratch folders, a scanner, and a fake TMDB.
    struct Fixture {
        scanner: Scanner,
        matcher: Matcher,
        contents: Contents,
        plugins: Registry,
        events: EventBus,
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
        async fn new() -> Self {
            let fixture = Self::without_tmdb().await;
            fixture.enable_tmdb();
            fixture
        }

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
            let events = EventBus::new(Arc::clone(&db), Registry::new(), ServerUrl::default());
            Self {
                scanner: Scanner::new(Arc::clone(&db), plugins.clone(), events.clone(), None),
                matcher: Matcher::new(Arc::clone(&db), plugins.clone(), events.clone()),
                contents: Contents::new(db),
                plugins,
                events,
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

        /// Fixes the Match of `item` by hand under its Library's scan lock, as the server does.
        async fn set_match(&self, item: i64, id: &ExternalId) -> Result<Summary, ManualError> {
            let scan = self.scanner.lock_library_of(item).await?;
            self.matcher.set_match(&scan, item, id).await
        }

        /// Clears the Match of `item` by hand under its Library's scan lock, as the server does.
        async fn clear_match(&self, item: i64) -> Result<Summary, ManualError> {
            let scan = self.scanner.lock_library_of(item).await?;
            self.matcher.clear_match(&scan, item).await
        }

        async fn items(&self, library: &Library) -> Vec<(String, FileItem)> {
            self.contents
                .files(library.id)
                .await
                .unwrap()
                .into_iter()
                .flat_map(|file| {
                    let relative = library.relative(&file.path);
                    file.items
                        .into_iter()
                        .map(move |item| (relative.clone(), item))
                })
                .collect()
        }

        /// What was stored for each item of `library`, as (path relative to its folder, Season
        /// and Episode, what it Matched).
        async fn stored(&self, library: &Library) -> Vec<(String, Option<(i64, i64)>, Outcome)> {
            self.items(library)
                .await
                .into_iter()
                .map(|(relative, stored)| {
                    let episode = match stored.item {
                        Item::Episode {
                            season, episode, ..
                        } => Some((season, episode)),
                        Item::Movie { .. } => None,
                    };
                    (relative, episode, Outcome::of(stored.recorded.as_ref()))
                })
                .collect()
        }

        async fn outcomes(&self, library: &Library) -> Vec<Outcome> {
            self.stored(library)
                .await
                .into_iter()
                .map(|(_, _, outcome)| outcome)
                .collect()
        }

        async fn first_recorded(&self, library: &Library) -> Recorded {
            self.items(library).await.remove(0).1.recorded.unwrap()
        }

        async fn item(&self, library: &Library, relative: &str) -> i64 {
            self.items(library)
                .await
                .into_iter()
                .find(|(path, _)| path == relative)
                .unwrap()
                .1
                .id
        }

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

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Outcome {
        /// Matched to the title with this TMDB id and name.
        Matched(i64, String),
        Unmatched,
        Undecided,
    }

    impl Outcome {
        fn of(recorded: Option<&Recorded>) -> Self {
            match recorded {
                Some(recorded) if recorded.matched => {
                    Self::Matched(recorded.ids.tmdb.unwrap(), recorded.title.clone().unwrap())
                }
                Some(_) => Self::Unmatched,
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
        // Two films called Hamlet came out in 2000; one has over ten times the other's votes.
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
                    matched(10_688, "Hamlet")
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
        let alien = media.first_recorded(movies).await;
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
    async fn tvdb_and_imdb_id_hints_match_without_a_search() {
        let media = Fixture::new().await;
        let (movies, shows) = (&media.movies, &media.shows);
        movies.write("Alien {imdb-tt0078748}/Alien.mkv");
        movies.write("Alien (1979) tt0078748/Alien (1979).mkv");
        // TMDB knows no title with this TVDB id.
        movies.write("Lost Reel (1950) [tvdbid-1]/Lost Reel (1950).mkv");
        shows.write("Dark [tvdbid-334824]/Season 01/Dark - S01E01.mkv");
        shows.write("Dark {tvdb-334824}/Dark - S01E02.mkv");
        // The IMDb id names a Movie, not a Show.
        shows.write("Alien (imdb=tt0078748)/Season 01/Alien - S01E01.mkv");

        media.scan(movies).await;
        media.scan(shows).await;

        assert_eq!(
            media.outcomes(movies).await,
            [
                matched(348, "Alien"),
                matched(348, "Alien"),
                Outcome::Unmatched
            ]
        );
        assert_eq!(
            media.outcomes(shows).await,
            [
                Outcome::Unmatched,
                matched(1_388_538, "Secrets"),
                matched(1_388_539, "Lies"),
            ]
        );
        assert_eq!(media.searches(), 0, "{:?}", media.tmdb.requests());
    }

    #[tokio::test]
    async fn a_newer_name_parser_matches_an_item_by_its_tvdb_id_hint() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        let pilot = "Dark [tvdbid-334824]/Season 01/Dark - S01E01.mkv";
        shows.write(pilot);
        media.scan(shows).await;
        // Before TVDB ids were read, the folder named a Show with no year and no ID Hint, which
        // a search left Unmatched.
        let (path, file) = media
            .contents
            .stored(shows.id)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let old_name = Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            hint: None,
            season: 1,
            episode: 1,
        };
        let reparse = Change::Reparse {
            file: file.id,
            path,
            items: vec![old_name],
        };
        media.contents.apply(shows.id, vec![reparse]).await.unwrap();
        let item = media.item(shows, pilot).await;
        media
            .contents
            .record(vec![(item, Decided::Unmatched(None))])
            .await
            .unwrap();
        assert_eq!(media.items(shows).await[0].1.item.name().hint, None);

        media.contents.read_by_older_parser(shows.id).await;
        media.scan(shows).await;

        assert_eq!(media.outcomes(shows).await, [matched(1_388_538, "Secrets")]);
    }

    #[tokio::test]
    async fn a_match_is_published_once_stored() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        movies.write("Alien (1979)/Alien (1979).mkv");
        // Unmatched, which publishes nothing.
        movies.write("Home Movies (2019)/Home Movies (2019).mkv");
        let filter = Filter {
            types: vec![EventType::EVENT_TYPE_ITEM_MATCHED],
            ..Filter::default()
        };
        let mut matches = Box::pin(media.events.subscribe_as_admin(filter));

        media.scan(movies).await;

        let alien = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;
        let expected = event(ItemMatched {
            library_id: movies.id,
            item_id: alien,
            title: "Alien".to_owned(),
            ..Default::default()
        });
        assert_eq!(
            matches.next().await.and_then(|response| response.message),
            Some(subscribe_response::Message::from(expected))
        );
        assert!(
            matches.next().now_or_never().is_none(),
            "an Unmatched item was published"
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
        let secrets = media.first_recorded(shows).await;
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
    async fn a_country_tag_on_a_show_is_searched_without_and_keeps_shows_from_that_country() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        // TMDB also has a Euphoria with no country, which is dropped.
        shows.write("Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv");
        // TheTVDB's UK is TMDB's GB.
        shows.write("The Office (UK)/Season 01/The Office (UK) - S01E01.mkv");
        shows.write("The Office (US)/Season 01/The Office (US) - S01E01.mkv");

        media.scan(shows).await;

        assert_eq!(
            media.outcomes(shows).await,
            [
                matched(1_734_395, "Pilot"),
                matched(2_996_001, "Downsize"),
                matched(2_316_001, "Pilot"),
            ]
        );
    }

    #[tokio::test]
    async fn a_show_left_unmatched_by_the_parser_before_country_tags_matches_once_rescanned() {
        let media = Fixture::without_tmdb().await;
        let shows = &media.shows;
        let pilot = "Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv";
        shows.write(pilot);
        media.scan(shows).await;
        let item = media.item(shows, pilot).await;
        media
            .contents
            .record(vec![(item, Decided::Unmatched(None))])
            .await
            .unwrap();
        media.contents.read_by_older_parser(shows.id).await;
        media.enable_tmdb();

        media.scan(shows).await;

        assert_eq!(media.outcomes(shows).await, [matched(1_734_395, "Pilot")]);
    }

    #[tokio::test]
    async fn a_bracket_that_is_not_a_country_stays_in_the_shows_title() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        shows.write("The Office (II)/Season 01/The Office (II) - S01E01.mkv");

        media.scan(shows).await;

        let requests = media.tmdb.requests();
        assert!(
            requests
                .iter()
                .any(|r| r.starts_with("/3/search/tv?") && r.contains("query=The+Office+%28II%29")),
            "{requests:?}"
        );
    }

    #[tokio::test]
    async fn a_country_tag_in_a_movies_name_stays_in_its_title() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        movies.write("Euphoria (US) (2019)/Euphoria (US) (2019).mkv");

        media.scan(movies).await;

        let requests = media.tmdb.requests();
        assert!(
            requests
                .iter()
                .any(|r| r.starts_with("/3/search/movie?") && r.contains("query=Euphoria+%28US%29")),
            "{requests:?}"
        );
    }

    #[tokio::test]
    async fn a_pin_on_a_show_named_with_a_country_tag_still_applies_after_a_newer_parser() {
        let media = Fixture::new().await;
        let shows = &media.shows;
        let pilot = "The Office (US)/Season 01/The Office (US) - S01E01.mkv";
        shows.write(pilot);
        media.scan(shows).await;
        let item = media.item(shows, pilot).await;
        media
            .set_match(item, &ExternalId::Tmdb(2996))
            .await
            .unwrap();
        let searches = media.searches();

        media.contents.read_by_older_parser(shows.id).await;
        media.scan(shows).await;

        assert_eq!(
            media.outcomes(shows).await,
            [matched(2_996_001, "Downsize")]
        );
        assert_eq!(media.searches(), searches);
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
        media
            .movies
            .write("Home Movies (2019)/Home Movies (2019).mkv");
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
    async fn a_newer_name_parser_retries_unmatched_items_and_keeps_automatic_matches() {
        let media = Fixture::new().await;
        media.movies.write("Alien (1979)/Alien (1979).mkv");
        media
            .movies
            .write("Home Movies (2019)/Home Movies (2019).mkv");
        media.scan(&media.movies).await;
        let alien = media
            .item(&media.movies, "Alien (1979)/Alien (1979).mkv")
            .await;
        let searches = media.searches();

        media.contents.read_by_older_parser(media.movies.id).await;
        media.scan(&media.movies).await;

        // Only Home Movies, left Unmatched, was searched for again: with its year, then without.
        assert_eq!(media.searches(), searches + 2);
        assert_eq!(
            media.outcomes(&media.movies).await,
            [matched(348, "Alien"), Outcome::Unmatched]
        );
        assert_eq!(
            media
                .item(&media.movies, "Alien (1979)/Alien (1979).mkv")
                .await,
            alien
        );
    }

    #[tokio::test]
    async fn a_newer_name_parser_matches_an_item_an_older_one_left_ambiguous() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        let hamlet = "Hamlet (2000)/Hamlet (2000).mkv";
        movies.write(hamlet);
        media.scan(movies).await;
        // Before votes broke a tie, the two Hamlets of 2000 left this file Unmatched.
        let item = media.item(movies, hamlet).await;
        media
            .contents
            .record(vec![(item, Decided::Unmatched(None))])
            .await
            .unwrap();

        media.contents.read_by_older_parser(movies.id).await;
        media.scan(movies).await;

        assert_eq!(media.outcomes(movies).await, [matched(10_688, "Hamlet")]);
    }

    #[tokio::test]
    async fn a_newer_name_parser_keeps_a_pinned_match() {
        let media = Fixture::new().await;
        let movies = &media.movies;
        let home_movies = "Home Movies (2019)/Home Movies (2019).mkv";
        movies.write(home_movies);
        media.scan(movies).await;
        let item = media.item(movies, home_movies).await;
        media.set_match(item, &ExternalId::Tmdb(949)).await.unwrap();
        let looked_up = media.tmdb.requests().len();

        media.contents.read_by_older_parser(movies.id).await;
        media.scan(movies).await;

        assert_eq!(media.outcomes(movies).await, [matched(949, "Heat")]);
        assert_eq!(media.tmdb.requests().len(), looked_up);
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

        media.set_match(item, &ExternalId::Tmdb(949)).await.unwrap();
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

        let summary = media.set_match(item, &ExternalId::Tmdb(949)).await.unwrap();

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
            .set_match(pilot, &ExternalId::Tmdb(2316))
            .await
            .unwrap();
        let us = media.outcomes(shows).await;
        media
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

        media.clear_match(item).await.unwrap();
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

        let result = media.set_match(item, &ExternalId::Tmdb(1)).await;

        assert!(
            matches!(result, Err(ManualError::NoSuchTitle(ExternalId::Tmdb(1)))),
            "{result:?}"
        );
        assert_eq!(media.outcomes(movies).await, [matched(348, "Alien")]);
    }

    #[tokio::test]
    async fn a_match_for_an_item_that_does_not_exist_is_refused() {
        let media = Fixture::new().await;

        let result = media.set_match(42, &ExternalId::Tmdb(949)).await;

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

        let result = media.set_match(item, &ExternalId::Tmdb(348)).await;

        assert!(
            matches!(result, Err(ManualError::Unavailable)),
            "{result:?}"
        );
    }
}
