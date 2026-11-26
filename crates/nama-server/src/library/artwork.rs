//! Posters, Backdrops, Stills and Title Logos, downloaded when a Match is made and kept in the image
//! store.
//!
//! A Match stores its title's images as paths a title source handed out, along with the Plugin
//! whose title source that was. Once a Match is stored, its images are downloaded through that
//! title source in the background, and kept in the [`ImageStore`] while some Match stores them. One
//! asked for before it arrived is downloaded on the spot. Images a search or lookup offered and no
//! Match stores go to a cache of their own, which can be deleted at any time (ADR 0037). Callers
//! should only ask for images some Match stores, or that a search or lookup offered to clients,
//! so clients can't use the store to fetch arbitrary images.
//!
//! Clients fetch them over plain HTTP at `/media/artwork/<plugin>/<path>` (ADR 0007: media bytes
//! are not RPC), which is served to anyone with a session. A title source's path names bytes that
//! never change, so a browser caches what it fetched there for good. Adding `?width=` asks for a
//! copy resized to one of [`crate::images::WIDTHS`] while an admin leaves resizing on (ADR 0039).

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    io,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    extract::{Path as UrlPath, Query, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use tokio::sync::{OwnedSemaphorePermit, RwLock, RwLockReadGuard, Semaphore};
use tokio_util::task::{TaskTracker, task_tracker::TaskTrackerToken};

use crate::{
    images::{
        Asked, BACKDROP_WIDTHS, ImageStore, KEPT_ARTWORK_DIR, MAX_SVG_BYTES, OFFERED_ARTWORK_DIR,
        POSTER_WIDTHS, Resize, SVG, TITLE_LOGO_WIDTHS, clean_svg, file_name, percent_decode,
    },
    library::contents::{Contents, ImageRef},
    library::custom_images::CustomImages,
    library::local_images::LocalImages,
    library::shown_images::InPlaceOfDownloaded,
    library::title_source::{self, ArtworkKind, Source, TitleKind, TitleSources},
    plugins::ExtensionPoints,
    server_settings::ImageSettings,
};

/// Where the images Matches store are kept.
const KEPT_DIR: &str = KEPT_ARTWORK_DIR;

/// Where the images only a search or lookup offered are kept, until someone deletes them.
// ponytail: nothing empties it; bound it like the resized copies' cache if it grows too big.
const OFFERED_DIR: &str = OFFERED_ARTWORK_DIR;

/// How many images are downloaded in the background at once. Each crosses from the title source as
/// one whole value, so this also bounds the memory downloads take on a 1 GiB machine.
const DOWNLOADS: usize = 2;

/// How many offered images are remembered before [`Artwork::offer`] forgets them all.
const MAX_OFFERED: usize = 10_000;

/// The language whose artwork is picked first.
// ponytail: fixed until a User's language is asked for; then the User's, where one is asked for.
const LANGUAGE: &str = "en";

/// The images [`Artwork::pick_for`] picks for a title, each with the Plugin whose source listed it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Picked {
    /// Its poster.
    pub(crate) poster: Option<ImageRef>,
    /// Its backdrop.
    pub(crate) backdrop: Option<ImageRef>,
    /// Its best Backdrop with text in [`LANGUAGE`], for clients that show it without a Title
    /// Logo over it. `None` if no Backdrop has text in that language.
    pub(crate) titled_backdrop: Option<ImageRef>,
    /// Its Title Logo.
    pub(crate) logo: Option<ImageRef>,
}

/// The Posters and Backdrops [`Artwork::listed`] offers for a title, each best first, at the URLs
/// clients fetch them at.
#[derive(Debug)]
pub(crate) struct Listed {
    pub(crate) posters: Vec<String>,
    pub(crate) backdrops: Vec<String>,
}

/// Stored images, fetched from title sources on first use.
#[derive(Debug, Clone)]
pub struct Artwork {
    store: ImageStore,
    metadata: TitleSources,
    /// The `(plugin, path)` of each image a search, lookup or artwork list handed out since the
    /// Server started.
    offered: Arc<Mutex<HashSet<(String, String)>>>,
    /// Taken by each background download, across every clone.
    downloads: Arc<Semaphore>,
    /// The background work of [`Self::download_missing`], and what [`Self::working`] marks as
    /// leading up to it, so a test can wait it out ([`Artwork::settled`]).
    tasks: TaskTracker,
    /// Written while unused images are deleted, and read while one is looked for, written or
    /// served, so a Match stored meanwhile never finds its image there and then has it deleted.
    sweeping: Arc<RwLock<()>>,
    /// What Posters, Backdrops and Title Logos are resized by once kept, so a first visit finds
    /// them made. `None` until startup hands it over (`Artwork::resizing_ahead`), so tests that
    /// build their own `Artwork` make copies on request only.
    ahead: Option<ResizingAhead>,
}

/// What [`Artwork`] resizes images ahead by.
#[derive(Debug, Clone)]
struct ResizingAhead {
    images: ImageSettings,
    /// With `local_images`, what titles show in place of their downloaded Posters, which so
    /// get no copy.
    custom_images: CustomImages,
    local_images: LocalImages,
}

/// What [`Artwork`] tracks its background work in: one tracker per Server, or under the `testing`
/// feature one per process, so [`Artwork::settled`] can reach the Server a test started.
fn background_tasks() -> TaskTracker {
    #[cfg(feature = "testing")]
    {
        static TASKS: std::sync::LazyLock<TaskTracker> = std::sync::LazyLock::new(TaskTracker::new);
        TASKS.clone()
    }
    #[cfg(not(feature = "testing"))]
    TaskTracker::new()
}

/// Why an image couldn't be had.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The title source has no such image.
    #[error("no such image")]
    NotFound,
    /// The Plugin isn't enabled or has no title source, or its title source failed. Worth trying
    /// again later.
    #[error("the image is unavailable: {0}")]
    Unavailable(String),
    /// The store couldn't be read or written.
    #[error("artwork store error: {0}")]
    Io(#[from] io::Error),
}

impl Artwork {
    /// The images kept in `store`, fetching through the title sources enabled in `plugins`.
    #[must_use]
    pub fn new(store: ImageStore, plugins: ExtensionPoints) -> Self {
        Self {
            store,
            metadata: TitleSources::new(plugins),
            offered: Arc::default(),
            downloads: Arc::new(Semaphore::new(DOWNLOADS)),
            tasks: background_tasks(),
            sweeping: Arc::default(),
            ahead: None,
        }
    }

    /// Marks the caller's sweep and [`Self::download_missing`] after it as under way until the
    /// token is dropped, so [`Artwork::settled`] waits for work that hasn't spawned yet. Taken
    /// before the event that tells clients the change is done, which a test waits on.
    pub(crate) fn working(&self) -> TaskTrackerToken {
        self.tasks.token()
    }

    /// Waits until no artwork work a scan, Matching or a correction began, through
    /// [`Self::working`] or [`Self::download_missing`], is left, including each download it
    /// started. Testing only. Process-wide, as every [`Artwork`] shares one tracker under the
    /// `testing` feature: integration tests run one per process under nextest.
    #[cfg(feature = "testing")]
    pub async fn settled() {
        let tasks = background_tasks();
        tasks.close();
        tasks.wait().await;
        tasks.reopen();
    }

    /// This, resizing each Poster, Backdrop and Title Logo of a Library it downloads images for to
    /// the widths the web app offers as `images` says, so a first visit to the Library finds them
    /// made, unless a title shows one of `custom_images` or `local_images` in a Poster's place.
    #[must_use]
    pub(crate) fn resizing_ahead(
        self,
        images: ImageSettings,
        custom_images: CustomImages,
        local_images: LocalImages,
    ) -> Self {
        Self {
            ahead: Some(ResizingAhead {
                images,
                custom_images,
                local_images,
            }),
            ..self
        }
    }

    /// Where clients fetch the image at `path` from the Plugin `plugin`'s title source, which
    /// [`router`] serves from now on even though no Match stores it.
    pub(crate) fn offer(&self, plugin: &PluginId, path: &str) -> String {
        let plugin = plugin.as_str();
        let mut offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        // ponytail: forgets every offer at the cap, so a poster shown long ago may 404 until it is
        // searched for again. Evict the oldest instead if that bites.
        if offered.len() >= MAX_OFFERED {
            offered.clear();
        }
        offered.insert((plugin.to_owned(), path.to_owned()));
        url(plugin, path)
    }

    /// The thumbhash of each image resized so far, by the URL clients fetch it at.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn thumbhashes(&self) -> sqlx::Result<HashMap<String, Vec<u8>>> {
        let mut by_url = HashMap::new();
        for dir in [OFFERED_DIR, KEPT_DIR] {
            let stored = self.store.thumbhashes(dir).await?;
            by_url.extend(
                (stored.into_iter())
                    .filter_map(|(key, thumbhash)| Some((url_of(&key)?, thumbhash))),
            );
        }
        Ok(by_url)
    }

    /// The artwork picked from the lists every source in `sources` has for the title of `kind`
    /// with the ids `ids`, as [`pick`] picks it.
    pub(crate) async fn pick_for(
        &self,
        sources: &TitleSources,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Picked {
        pick(&sources.artwork_lists(kind, ids).await, LANGUAGE)
    }

    /// The Posters and Backdrops every enabled source's artwork list has for the title of `kind`
    /// with the id `id`, or with `season` only the Posters of that Season of the Show, ranked as
    /// [`pick`] ranks them, each offered. Asked anew each call.
    pub(crate) async fn listed(
        &self,
        kind: TitleKind,
        id: &ExternalId,
        season: Option<u32>,
    ) -> Listed {
        let ids = ExternalIds::from_iter([id.clone()]);
        let offered = |lists: &[(Source, Vec<title_source::Artwork>)], of_kind| {
            (ranked(lists, of_kind, LANGUAGE).into_iter())
                .map(|(_, source, art)| self.offer(&source.plugin, &art.path))
                .collect()
        };
        let Some(season) = season else {
            let lists = self.metadata.artwork_lists(kind, &ids).await;
            return Listed {
                posters: offered(&lists, ArtworkKind::Poster),
                backdrops: offered(&lists, ArtworkKind::Backdrop),
            };
        };
        let lists = match kind {
            TitleKind::Show => self.metadata.season_artwork_lists(&ids, season).await,
            // A Movie has no Seasons.
            TitleKind::Movie => Vec::new(),
        };
        // A Season's one image is its Poster.
        Listed {
            posters: offered(&lists, ArtworkKind::Poster),
            backdrops: Vec::new(),
        }
    }

    /// The bytes of the image at `url`, which must be one [`Self::offer`] gave, fetched through
    /// the title source that listed it: never from an address a client names.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if `url` isn't an offered image or the source no longer has it, and
    /// [`Error::Unavailable`] if the source can't be reached.
    pub(super) async fn offered_bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        let (plugin, path) = url
            .strip_prefix("/media/artwork/")
            .and_then(|rest| rest.split_once('/'))
            .and_then(|(plugin, path)| Some((percent_decode(plugin)?, percent_decode(path)?)))
            .filter(|(plugin, path)| self.was_offered(plugin, path))
            .ok_or(Error::NotFound)?;
        Ok(self.fetch(&PluginId::from(plugin), &path).await?.bytes)
    }

    pub(super) fn was_offered(&self, plugin: &str, path: &str) -> bool {
        let offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        offered.contains(&(plugin.to_owned(), path.to_owned()))
    }

    /// Stores the image at `path` from the Plugin `plugin`'s title source, downloading it unless
    /// it is stored already.
    ///
    /// # Errors
    ///
    /// If the title source has no such image, can't be reached, or the store can't be written.
    pub async fn keep(&self, plugin: &PluginId, path: &str) -> Result<(), Error> {
        self.store_in(KEPT_DIR, plugin, path).await.map(drop)
    }

    /// Downloads, in the background, each image a Match in the Library `library` stores that
    /// isn't kept yet, logging those that fail, which the Library's next scan tries again. Each
    /// Poster a title of it shows, downloaded before or just now or beside its media, and each
    /// downloaded Backdrop and Title Logo, is resized to the widths the web app offers it at too,
    /// where resizing is on now and those copies aren't there.
    pub(crate) fn download_missing(&self, contents: Contents, library: i64) {
        let artwork = self.clone();
        // Read before any download lands, so a change of settings after the scan has no part in it.
        let copies = (self.ahead.as_ref()).map_or_else(CopiesAhead::default, CopiesAhead::of);
        if let Some(ahead) = &self.ahead
            && !copies.posters.is_empty()
        {
            let posters = copies.posters.clone();
            (ahead.local_images).resize_shown_posters(
                library,
                ahead.custom_images.clone(),
                posters,
            );
        }
        self.tasks.spawn(async move {
            let listed = tokio::try_join!(
                contents.images(Some(library)),
                artwork.shown_posters(&contents, library),
                contents.backdrops_and_title_logos(library)
            );
            match listed {
                Ok((images, posters, (backdrops, title_logos))) => {
                    // A Poster that is also a Backdrop is rare enough to get only a Poster's.
                    let ahead = (backdrops.into_iter())
                        .map(|image| (image, &*copies.backdrops))
                        .chain((title_logos.into_iter()).map(|image| (image, &*copies.title_logos)))
                        .chain(posters.into_iter().map(|image| (image, &*copies.posters)))
                        .collect();
                    artwork.download(images, &ahead).await;
                }
                Err(err) => {
                    tracing::error!(library, "could not list the images to download: {err}");
                }
            }
        });
    }

    /// Each Poster, as its Plugin and path, that some title of the Library `library` shows, not
    /// having a Custom Image or image beside its media in its place. None without
    /// [`Self::resizing_ahead`].
    async fn shown_posters(
        &self,
        contents: &Contents,
        library: i64,
    ) -> sqlx::Result<HashSet<(String, String)>> {
        let Some(ahead) = &self.ahead else {
            return Ok(HashSet::new());
        };
        let (posters, in_place) = tokio::try_join!(
            contents.posters(library),
            InPlaceOfDownloaded::read(&ahead.custom_images, &ahead.local_images)
        )?;
        Ok((posters.into_iter())
            .filter(|poster| in_place.shows_downloaded(&poster.ids, poster.image))
            .map(|poster| (poster.plugin, poster.path))
            .collect())
    }

    /// Downloads each of `images`, a Plugin and a path, unless it is kept already, and makes the
    /// copies `ahead` lists of each, kept already or not, at most [`DOWNLOADS`] at a time across
    /// the Server. Returns once the last has started.
    async fn download(
        &self,
        images: Vec<(String, String)>,
        ahead: &HashMap<(String, String), &[Resize]>,
    ) {
        for image in images {
            // The semaphore is never closed.
            let Ok(permit) = Arc::clone(&self.downloads).acquire_owned().await else {
                return;
            };
            let copies = ahead
                .get(&image)
                .map_or_else(Vec::new, |copies| copies.to_vec());
            self.tasks
                .spawn(self.clone().download_one(image, copies, permit));
        }
    }

    /// Keeps one image for [`Self::download`], logging a failure, and makes `copies` of it, then
    /// lets the next start. The copies are made under the download's permit, so those of at most
    /// [`DOWNLOADS`] images wait on the resize thread ahead of a request's.
    async fn download_one(
        self,
        (plugin, path): (String, String),
        copies: Vec<Resize>,
        _permit: OwnedSemaphorePermit,
    ) {
        match self
            .store_in(KEPT_DIR, &PluginId::from(plugin.clone()), &path)
            .await
        {
            Err(err) => tracing::warn!(path, "could not download an image: {err}"),
            Ok(sweeping) => {
                drop(sweeping);
                let key = key(KEPT_DIR, &plugin, &path);
                for copy in copies {
                    // Held through each width's copies, as a request's resize holds it, so a sweep
                    // deleting the image meanwhile can't leave a copy and its thumbhash behind,
                    // yet waits on one width's, not all of them, with image requests behind it.
                    let _sweeping = self.sweeping.read().await;
                    self.store.copy_ahead(&key, &[copy]).await;
                }
            }
        }
    }

    /// Deletes each kept image no Match in `contents` stores any more, logging a failure.
    pub(crate) async fn drop_unused(&self, contents: &Contents) {
        if let Err(err) = self.try_drop_unused(contents).await {
            tracing::error!("could not delete the images no Match stores: {err}");
        }
    }

    async fn try_drop_unused(
        &self,
        contents: &Contents,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _sweeping = self.sweeping.write().await;
        // ponytail: lists every kept image and every Match on each scan and correction; delete
        // by what the change removed instead if that gets slow on a big store.
        let kept = self.store.list(KEPT_DIR).await?;
        let used: HashSet<(String, String)> = contents.images(None).await?.into_iter().collect();
        for key in kept {
            let [_, plugin, path] = key.as_slice() else {
                continue;
            };
            if !used.contains(&(plugin.clone(), path.clone())) {
                self.store.delete(&[KEPT_DIR, plugin, path]).await?;
            }
        }
        Ok(())
    }

    /// Stores the image at `path` from `plugin`'s title source in `dir`, downloading it unless it
    /// is stored there already. Returns holding off [`Self::drop_unused`], so the image stays until
    /// the guard is dropped.
    async fn store_in(
        &self,
        dir: &str,
        plugin: &PluginId,
        path: &str,
    ) -> Result<RwLockReadGuard<'_, ()>, Error> {
        let key = key(dir, plugin.as_str(), path);
        // The lock isn't held across the download: a sweep waiting on a slow title source would
        // hold up the scan that runs it, and every image request queued behind the sweep. A sweep
        // meanwhile can't delete what isn't stored yet. Two downloads of one image racing here
        // store it once, as `ImageStore::write` keeps the first.
        // ponytail: an image downloaded while its Match is corrected stays until the next sweep.
        {
            let sweeping = self.sweeping.read().await;
            if self.store.contains(&key).await? {
                return Ok(sweeping);
            }
        }
        let mut bytes = self.fetch(plugin, path).await?.bytes;
        if content_type(path) == SVG {
            bytes = cleaned_svg(plugin, &bytes)?;
        }
        let sweeping = self.sweeping.read().await;
        self.store.write(&key, &bytes).await?;
        Ok(sweeping)
    }

    /// The image at `path` from `plugin`'s title source, which handed the path out.
    async fn fetch(&self, plugin: &PluginId, path: &str) -> Result<title_source::Image, Error> {
        self.metadata
            .image(plugin, path)
            .await
            .map_err(|err| match err {
                title_source::Unanswered::NotFound => Error::NotFound,
                title_source::Unanswered::Unavailable => Error::Unavailable(format!(
                    "{plugin} isn't enabled, doesn't provide metadata, or failed"
                )),
            })
    }

    /// The image at `path` from `plugin`'s title source as the answer to `request`, stored first if
    /// it isn't yet.
    async fn serve_image(
        &self,
        (dir, plugin, path): (&str, &str, &str),
        resize: Option<Resize>,
        request: Request,
    ) -> Result<Response, Error> {
        let _sweeping = self
            .store_in(dir, &PluginId::from(plugin.to_owned()), path)
            .await?;
        Ok(self
            .store
            .serve(&key(dir, plugin, path), content_type(path), resize, request)
            .await?)
    }
}

/// The copies Posters are resized to ahead of the Poster grid asking for them, as `images` says:
/// none while resizing is off.
pub(super) fn poster_copies(images: &ImageSettings) -> Vec<Resize> {
    Resize::each(&POSTER_WIDTHS, images)
}

/// The copies [`Artwork::download_missing`] makes of each kind of image ahead of the web app
/// asking for them.
#[derive(Debug, Default)]
struct CopiesAhead {
    posters: Vec<Resize>,
    backdrops: Vec<Resize>,
    title_logos: Vec<Resize>,
}

impl CopiesAhead {
    /// Those `ahead` makes as its settings are now: none while resizing is off.
    fn of(ahead: &ResizingAhead) -> Self {
        Self {
            posters: poster_copies(&ahead.images),
            backdrops: Resize::each(&BACKDROP_WIDTHS, &ahead.images),
            title_logos: Resize::each(&TITLE_LOGO_WIDTHS, &ahead.images),
        }
    }
}

/// An image of an artwork list, with its source and that source's place in the order.
type Ranked<'a> = (usize, &'a Source, &'a title_source::Artwork);

/// The best image of each kind in `lists`, which come in the sources' order, as [`ranked`] ranks
/// them, and the best Backdrop in `language`.
fn pick(lists: &[(Source, Vec<title_source::Artwork>)], language: &str) -> Picked {
    let image = |(_, source, art): Ranked<'_>| ImageRef {
        plugin: source.plugin.clone(),
        path: art.path.clone(),
    };
    let best = |kind| ranked(lists, kind, language).into_iter().next().map(image);
    let backdrops = ranked(lists, ArtworkKind::Backdrop, language);
    // Among Backdrops in one language, `ranked` already orders by source, rating, then votes.
    let titled_backdrop = (backdrops.iter().copied())
        .find(|(_, _, art)| {
            (art.language.as_deref()).is_some_and(|lang| same_language(lang, language))
        })
        .map(image);
    Picked {
        poster: best(ArtworkKind::Poster),
        backdrop: backdrops.first().copied().map(image),
        titled_backdrop,
        logo: best(ArtworkKind::Logo),
    }
}

/// The images of `kind` in `lists`, which come in the sources' order, best first. Images rank by
/// their language: `language` first, then none, then any other, except Backdrops, which rank none
/// first, then `language`; then by their source's place in the order; then by higher rating,
/// then more votes; then by their place in their source's list.
fn ranked<'a>(
    lists: &'a [(Source, Vec<title_source::Artwork>)],
    kind: ArtworkKind,
    language: &str,
) -> Vec<Ranked<'a>> {
    // A Backdrop sits behind the app's own text, and title sources mark textless art by giving
    // it no language.
    let language_rank = |art: &title_source::Artwork| match (art.kind, art.language.as_deref()) {
        (ArtworkKind::Backdrop, None) => 0,
        (ArtworkKind::Backdrop, Some(lang)) if same_language(lang, language) => 1,
        (_, Some(lang)) if same_language(lang, language) => 0,
        (_, None) => 1,
        (_, Some(_)) => 2,
    };
    let rank = |(a_place, _, a): &Ranked<'_>, (b_place, _, b): &Ranked<'_>| -> Ordering {
        let rating = |art: &title_source::Artwork| art.rating.unwrap_or(f64::NEG_INFINITY);
        (language_rank(a).cmp(&language_rank(b)))
            .then(a_place.cmp(b_place))
            .then(rating(b).total_cmp(&rating(a)))
            .then(b.votes.unwrap_or(0).cmp(&a.votes.unwrap_or(0)))
    };
    let mut of_kind: Vec<Ranked<'a>> = lists
        .iter()
        .enumerate()
        .flat_map(|(place, (source, list))| {
            list.iter()
                .filter(move |art| art.kind == kind)
                .map(move |art| (place, source, art))
        })
        .collect();
    // A stable sort keeps equals in their source's list order.
    of_kind.sort_by(rank);
    of_kind
}

/// Whether an image's `lang` is the `language` asked for, ignoring ASCII case.
fn same_language(lang: &str, language: &str) -> bool {
    lang.eq_ignore_ascii_case(language)
}

/// Where the store keeps `plugin`'s image at `path` in `dir`. Under [`KEPT_DIR`] it is where images
/// were kept before the store existed, so changing it downloads every image again.
fn key<'a>(dir: &'a str, plugin: &'a str, path: &'a str) -> [&'a str; 3] {
    [dir, plugin, path]
}

/// Where clients fetch the image the store keeps at `key`, in either folder.
fn url_of(key: &[String]) -> Option<String> {
    let [_, plugin, path] = key else {
        return None;
    };
    Some(url(plugin, path))
}

/// Where clients fetch the image: `/media/artwork/<plugin>/<path>`, each part one URL path segment
/// (escaped as [`file_name`] does, which decodes back to what it was).
pub(crate) fn url(plugin: &str, path: &str) -> String {
    format!("/media/artwork/{}/{}", file_name(plugin), file_name(path))
}

/// Serves `GET` [`url()`]s, 404 unless `artwork` offered the image or `contents` has a Match that
/// stores it, resized as `images` says. Whether the caller has a session is up to the layers the
/// router is put behind.
pub(crate) fn router(artwork: Artwork, contents: Contents, images: ImageSettings) -> Router {
    Router::new()
        .route("/media/artwork/{plugin}/{path}", get(serve))
        .with_state((artwork, contents, images))
}

async fn serve(
    State((artwork, contents, images)): State<(Artwork, Contents, ImageSettings)>,
    UrlPath((plugin, path)): UrlPath<(String, String)>,
    Query(asked): Query<Asked>,
    request: Request,
) -> Response {
    let resize = match asked.resize(&images) {
        Ok(resize) => resize,
        Err(refused) => return refused.into_response(),
    };
    let dir = match contents.stores_image(&plugin, &path).await {
        Ok(true) => KEPT_DIR,
        Ok(false) if artwork.was_offered(&plugin, &path) => OFFERED_DIR,
        Ok(false) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up a stored image: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    match artwork
        .serve_image((dir, &plugin, &path), resize, request)
        .await
    {
        Ok(response) => response,
        Err(Error::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ Error::Unavailable(_)) => {
            tracing::warn!("could not serve {path}: {err}");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(err @ Error::Io(_)) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `svg` as [`clean_svg`] leaves it, if it is at most [`MAX_SVG_BYTES`] and an SVG document: a
/// second guard beside the headers `ImageStore::serve` sends an SVG with. Logs why it refuses one,
/// which then answers as an image the title source doesn't have.
fn cleaned_svg(plugin: &PluginId, svg: &[u8]) -> Result<Vec<u8>, Error> {
    let why = if svg.len() > MAX_SVG_BYTES {
        format!("it is over {MAX_SVG_BYTES} bytes")
    } else {
        match clean_svg(svg) {
            Ok(cleaned) => return Ok(cleaned),
            Err(err) => format!("it isn't SVG: {err}"),
        }
    };
    tracing::warn!(plugin = %plugin, "dropped an SVG image: {why}");
    Err(Error::NotFound)
}

fn content_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        // A title source's Title Logo may be one (#2016); `ImageStore::serve` keeps its script
        // from running.
        Some("svg") => SVG,
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
#[path = "artwork_tests.rs"]
mod tests;
