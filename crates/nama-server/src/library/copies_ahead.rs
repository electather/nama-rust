//! Copies ahead: the resized copies of each image a Library shows, made at the widths the web app
//! offers it at before any client asks, so a first visit resizes nothing on request (ADR 0039).
//!
//! Which images get copies, and at which widths, is decided here alone: [`listed`] turns what
//! titles show in place of their downloaded images ([`InPlaceOfDownloaded`]) and a Library's rows
//! into an ordered list. Callers make one call naming what happened, and each returns at once: a
//! Library's Matches changed ([`CopiesAhead::matches_changed`]), or a Custom Image slot did
//! ([`CopiesAhead::slot_changed`]). Runs take turns, one at a time across the Server, so at most
//! one run's copies wait on the resize thread ahead of a request's (ADR 0039).

use std::{collections::HashSet, iter, sync::Arc};

use nama_plugin_api::metadata::ExternalIds;
use tokio::sync::Mutex;
use tokio_util::task::{TaskTracker, task_tracker::TaskTrackerToken};

use crate::{
    db::Database,
    images::{BACKDROP_WIDTHS, ImageStore, POSTER_WIDTHS, Resize, TITLE_LOGO_WIDTHS},
    library::{
        Role, Slot,
        artwork::{self, Artwork},
        contents::{Contents, LocalRole, StoredTitleImage, TitleLocalImage},
        custom_images::{CustomImages, Subject},
        libraries::Libraries,
        local_images::{self, LocalImageUrls},
        shown_images::{InPlaceOfDownloaded, TitleImage},
    },
    server_settings::ImageSettings,
};

/// Makes the copies ahead of every image a Library shows.
#[derive(Debug, Clone)]
pub(crate) struct CopiesAhead {
    images: ImageSettings,
    contents: Contents,
    libraries: Libraries,
    artwork: Artwork,
    custom_images: CustomImages,
    store: ImageStore,
    /// Held by each run from reading its list until its copies are made: the queue runs wait their
    /// turn in, so a run lists what is shown once the one before it is done.
    queue: Arc<Mutex<()>>,
    /// Artwork's, so its test-only wait (`Artwork::settled`) covers every run too.
    tasks: TaskTracker,
}

/// What an image is shown as, which says the widths its copies are made at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Poster,
    Backdrop,
    TitleLogo,
}

/// An image the store keeps.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Image {
    /// A downloaded one, by the Plugin whose title source handed out its path.
    Downloaded {
        plugin: String,
        path: String,
    },
    /// One beside the media, by its Library and the name its copy is stored under.
    BesideMedia {
        library: i64,
        stored: String,
    },
    Custom {
        slot: Slot,
        version: String,
    },
}

/// One image to copy ahead, with the copies to make of it.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CopyAhead {
    pub(super) image: Image,
    pub(super) kind: Kind,
    pub(super) copies: Vec<Resize>,
}

/// What a Library's list is made from, read once.
#[derive(Debug, Default)]
pub(super) struct Rows {
    library: i64,
    /// The Posters its Matches store, with their titles' ids.
    posters: Vec<StoredTitleImage>,
    /// The Backdrops its Matches store, with their titles' ids.
    backdrops: Vec<StoredTitleImage>,
    titled_backdrops: Vec<(String, String)>,
    title_logos: Vec<(String, String)>,
    trailer_thumbnails: Vec<(String, String)>,
    /// Every Library's images beside the media: only this one's are listed.
    beside_media: Vec<TitleLocalImage>,
    /// The Custom Images of the Library and its titles, as slot and version, its own Custom
    /// Backdrop among them.
    custom: Vec<(Slot, String)>,
}

/// An image a Library shows, and where a Custom Image would stand in for it.
struct Shown<'a> {
    image: Image,
    kind: Kind,
    at: At<'a>,
}

/// Which Custom Image slots show an image.
enum At<'a> {
    /// Each slot of the title with every id it has, for this image of it.
    Title(&'a ExternalIds, TitleImage),
    Slot(&'a Slot),
    /// No slot: a titled Backdrop, Title Logo or Trailer thumbnail has none (#1339).
    Nowhere,
}

impl Kind {
    /// The widths the web app offers an image of this kind at, and so makes it at ahead.
    const fn widths(self) -> &'static [u32] {
        match self {
            Self::Poster => &POSTER_WIDTHS,
            Self::Backdrop => &BACKDROP_WIDTHS,
            Self::TitleLogo => &TITLE_LOGO_WIDTHS,
        }
    }
}

impl At<'_> {
    fn is(&self, slot: &Slot) -> bool {
        match self {
            Self::Title(ids, image) => ids.iter().any(|id| image.slot(&id) == *slot),
            Self::Slot(at) => *at == slot,
            Self::Nowhere => false,
        }
    }
}

/// The images the Library of `rows` shows, as `in_place` says what titles show in place of their
/// downloaded ones, each with its copies in a cache of `cache_limit` bytes, none while resizing is
/// off.
pub(super) fn listed(
    rows: &Rows,
    in_place: &InPlaceOfDownloaded,
    cache_limit: Option<u64>,
) -> Vec<CopyAhead> {
    ordered(shown(rows, in_place), cache_limit)
}

/// Of [`listed`], only the images `slot` shows: its Custom Image, or the one its title shows
/// without it.
pub(super) fn listed_at(
    slot: &Slot,
    rows: &Rows,
    in_place: &InPlaceOfDownloaded,
    cache_limit: Option<u64>,
) -> Vec<CopyAhead> {
    let shown = shown(rows, in_place).into_iter();
    ordered(shown.filter(|shown| shown.at.is(slot)), cache_limit)
}

/// Each image the Library of `rows` shows, but none a title shows another in place of. A Library's
/// own Custom Backdrop hides none, since each title's page still shows the title's Backdrop.
fn shown<'a>(rows: &'a Rows, in_place: &InPlaceOfDownloaded) -> Vec<Shown<'a>> {
    let stored = (rows.posters.iter().map(|stored| (stored, Kind::Poster)))
        .chain(rows.backdrops.iter().map(|stored| (stored, Kind::Backdrop)))
        .filter(|(stored, _)| in_place.shows_downloaded(&stored.ids, stored.image))
        .map(|(stored, kind)| Shown {
            image: downloaded(&stored.plugin, &stored.path),
            kind,
            at: At::Title(&stored.ids, stored.image),
        });
    let beside_media = (rows.beside_media.iter())
        .filter(|image| image.library == rows.library)
        .filter_map(|image| {
            let shown_as = image.shown_as()?;
            let url = local_images::url(image.library, &image.stored);
            (in_place.shows_beside_media(&image.ids, shown_as, &url)).then(|| Shown {
                image: Image::BesideMedia {
                    library: image.library,
                    stored: image.stored.clone(),
                },
                kind: match image.role {
                    LocalRole::Title(Role::Backdrop) => Kind::Backdrop,
                    LocalRole::Title(Role::Poster) | LocalRole::SeasonPoster => Kind::Poster,
                },
                at: At::Title(&image.ids, shown_as),
            })
        });
    let custom = rows.custom.iter().map(|(slot, version)| Shown {
        image: Image::Custom {
            slot: slot.clone(),
            version: version.clone(),
        },
        kind: if slot.is_poster() {
            Kind::Poster
        } else {
            Kind::Backdrop
        },
        at: At::Slot(slot),
    });
    // A Trailer's thumbnail is shown as wide as a Backdrop.
    let untitled = (rows.titled_backdrops.iter().chain(&rows.trailer_thumbnails))
        .map(|image| (image, Kind::Backdrop))
        .chain(
            rows.title_logos
                .iter()
                .map(|image| (image, Kind::TitleLogo)),
        )
        .map(|((plugin, path), kind)| Shown {
            image: downloaded(plugin, path),
            kind,
            at: At::Nowhere,
        });
    stored
        .chain(beside_media)
        .chain(custom)
        .chain(untitled)
        .collect()
}

fn downloaded(plugin: &str, path: &str) -> Image {
    Image::Downloaded {
        plugin: plugin.to_owned(),
        path: path.to_owned(),
    }
}

/// `shown`, every Poster first, each image once, with its copies.
fn ordered<'a>(
    shown: impl IntoIterator<Item = Shown<'a>>,
    cache_limit: Option<u64>,
) -> Vec<CopyAhead> {
    let mut shown: Vec<_> = shown.into_iter().collect();
    // Before any Backdrop or Title Logo, so a cache near its limit holds what a first visit needs
    // (#2253). A stable sort keeps the rest in the Library's order.
    shown.sort_by_key(|shown| shown.kind != Kind::Poster);
    // A Poster that is also a Backdrop is rare enough to get only a Poster's copies.
    let mut listed = HashSet::new();
    (shown.into_iter())
        .filter(|shown| listed.insert(shown.image.clone()))
        .map(|shown| CopyAhead {
            copies: Resize::each(shown.kind.widths(), cache_limit),
            image: shown.image,
            kind: shown.kind,
        })
        .collect()
}

impl CopiesAhead {
    /// Copies ahead of the images recorded in `db` and kept in `store`, those Matches store
    /// downloaded through `artwork`, as `images` says.
    pub(crate) fn new(
        db: Arc<Database>,
        store: ImageStore,
        artwork: Artwork,
        images: ImageSettings,
    ) -> Self {
        Self {
            images,
            contents: Contents::new(Arc::clone(&db)),
            libraries: Libraries::new(Arc::clone(&db)),
            custom_images: CustomImages::new(db, store.clone()),
            tasks: artwork.tasks().clone(),
            artwork,
            store,
            queue: Arc::default(),
        }
    }

    /// As [`Self::new`], downloading through the title sources `plugins` enables, with the
    /// default image settings, for a test that builds a Scanner or Matcher.
    #[cfg(test)]
    pub(crate) fn for_tests(
        db: &Arc<Database>,
        store: ImageStore,
        plugins: crate::plugins::ExtensionPoints,
    ) -> Self {
        let settings =
            crate::server_settings::Settings::new(Arc::clone(db), std::path::PathBuf::new());
        let artwork = Artwork::new(store.clone(), plugins);
        Self::new(Arc::clone(db), store, artwork, settings.images())
    }

    /// The Artwork it downloads through, which also picks a Match's artwork and deletes what no
    /// Match stores.
    pub(crate) const fn artwork(&self) -> &Artwork {
        &self.artwork
    }

    /// Marks the caller's work, and the run it then asks for, as under way until the token is
    /// dropped, so `Artwork::settled` waits for a run that hasn't started yet. Taken before the
    /// event that tells clients the change is done, which a test waits on.
    pub(crate) fn working(&self) -> TaskTrackerToken {
        self.tasks.token()
    }

    /// Once a change that stores or removes the Library `library`'s Matches commits: downloads,
    /// in the background, each image they store that isn't kept yet, logging those that fail,
    /// which the next scan tries again, and makes the copies of every image the Library shows
    /// that aren't there, each one, so a Library recorded before copies were made ahead, or whose
    /// copies were evicted, gets them (#2152, #2372). Downloads go Posters first, resizing on or
    /// off.
    pub(crate) fn matches_changed(&self, library: i64) {
        let this = self.clone();
        // Read now, so a change of settings after the change has no part in it.
        let cache_limit = self.images.resizing();
        (self.tasks).spawn(async move { this.copy_library(library, cache_limit).await });
    }

    /// Once an upload to or removal from `slot` commits: makes, in the background, the copies of
    /// the image it now shows, in each Library that has its title. A downloaded one not kept yet
    /// gets them at the next scan. Nothing while resizing is off.
    pub(super) fn slot_changed(&self, slot: Slot) {
        let Some(cache_limit) = self.images.resizing() else {
            return;
        };
        let this = self.clone();
        (self.tasks).spawn(async move { this.copy_slot(slot, Some(cache_limit)).await });
    }

    async fn copy_library(&self, library: i64, cache_limit: Option<u64>) {
        let turn = self.queue.lock().await;
        let read = tokio::try_join!(self.read(library), self.contents.images(Some(library)));
        let ((rows, in_place), stored) = match read {
            Ok(read) => read,
            Err(err) => {
                tracing::error!(library, "could not list the images to download: {err}");
                return;
            }
        };
        let list = listed(&rows, &in_place, cache_limit);
        let shown_downloaded: HashSet<(&str, &str)> = (list.iter())
            .filter_map(|ahead| match &ahead.image {
                Image::Downloaded { plugin, path } => Some((plugin.as_str(), path.as_str())),
                Image::BesideMedia { .. } | Image::Custom { .. } => None,
            })
            .collect();
        let unlisted: Vec<_> = (stored.into_iter())
            .filter(|(plugin, path)| !shown_downloaded.contains(&(plugin.as_str(), path.as_str())))
            .map(|image| (image, Vec::new()))
            .collect();
        let (posters, others): (Vec<_>, Vec<_>) = list
            .into_iter()
            .partition(|ahead| ahead.kind == Kind::Poster);
        for round in [posters, others] {
            self.run_round(round).await;
        }
        drop(turn);
        // Kept while a Match stores it, though no title shows it, such as one a Custom Image hides.
        self.artwork.download(unlisted).await;
    }

    /// Makes `round`'s copies of the images kept already while Artwork downloads the others, each
    /// making its copies once it lands.
    async fn run_round(&self, round: Vec<CopyAhead>) {
        let (mut downloads, mut kept) = (Vec::new(), Vec::new());
        for ahead in round {
            match ahead.image {
                Image::Downloaded { plugin, path } => {
                    downloads.push(((plugin, path), ahead.copies));
                }
                Image::BesideMedia { .. } | Image::Custom { .. } => kept.push(ahead),
            }
        }
        tokio::join!(self.copy(&kept), self.artwork.download(downloads));
    }

    async fn copy_slot(&self, slot: Slot, cache_limit: Option<u64>) {
        let _turn = self.queue.lock().await;
        match self.slot_list(&slot, cache_limit).await {
            Ok(list) => self.copy(&list).await,
            Err(err) => tracing::error!("could not list the images a Custom Image changed: {err}"),
        }
    }

    /// [`listed_at`] `slot` in each Library that has its title, or that it is the Backdrop of,
    /// each image once.
    // ponytail: reads every Library's images for a title's slot, as an admin changes one. Read
    // only the Libraries with the title if that gets slow.
    async fn slot_list(
        &self,
        slot: &Slot,
        cache_limit: Option<u64>,
    ) -> Result<Vec<CopyAhead>, Box<dyn std::error::Error + Send + Sync>> {
        let libraries = match slot.library_id() {
            Some(library) => vec![library],
            None => (self.libraries.list().await?.into_iter())
                .map(|library| library.id)
                .collect(),
        };
        let mut list = Vec::new();
        for library in libraries {
            let (rows, in_place) = self.read(library).await?;
            list.extend(listed_at(slot, &rows, &in_place, cache_limit));
        }
        let mut listed = HashSet::new();
        list.retain(|ahead| listed.insert(ahead.image.clone()));
        Ok(list)
    }

    /// The Library `library`'s rows, and what titles show in place of their downloaded images.
    async fn read(&self, library: i64) -> sqlx::Result<(Rows, InPlaceOfDownloaded)> {
        let contents = &self.contents;
        let (
            posters,
            backdrops,
            (titled_backdrops, title_logos),
            trailer_thumbnails,
            beside_media,
            urls,
            custom,
        ) = tokio::try_join!(
            contents.posters(library),
            contents.backdrops(library),
            contents.titled_backdrops_and_title_logos(library),
            contents.trailer_thumbnails(library),
            contents.titles_local_images(),
            self.custom_images.urls(),
            self.custom_of(library),
        )?;
        // Filed from the rows just read, which `InPlaceOfDownloaded::read` would read again.
        let in_place = InPlaceOfDownloaded::new(urls, LocalImageUrls::filed(&beside_media));
        let rows = Rows {
            library,
            posters,
            backdrops,
            titled_backdrops,
            title_logos,
            trailer_thumbnails,
            beside_media,
            custom,
        };
        Ok((rows, in_place))
    }

    /// Each Custom Image, as its slot and version, of the Library `library` or a Movie, Show or
    /// Season it has, kept under any of the title's ids.
    // ponytail: reads every Custom Image, as `CustomImages::urls` does, once a run. Query by slot
    // if that ever grows.
    pub(super) async fn custom_of(&self, library: i64) -> sqlx::Result<Vec<(Slot, String)>> {
        let stored = self.custom_images.stored().await?;
        let (mut movie_ids, mut show_ids) = (Vec::new(), Vec::new());
        for (slot, _) in &stored {
            match slot.subject() {
                Some(Subject::Movie(id)) => movie_ids.push(id),
                Some(Subject::Show(id) | Subject::Season(id, _)) => show_ids.push(id),
                Some(Subject::Library) | None => {}
            }
        }
        // Only the titles some Custom Image names, not the whole Library.
        let (movies, shows) = tokio::try_join!(
            self.contents.movies(library, Some(&movie_ids)),
            self.contents.shows(library, Some(&show_ids))
        )?;
        let mut owned = HashSet::from([Slot::library(library)]);
        for movie in movies {
            for id in iter::once(movie.id).chain(movie.ids.iter()) {
                owned.extend(Role::ALL.map(|role| Slot::movie(&id, role)));
            }
        }
        for show in shows {
            for id in iter::once(show.id).chain(show.ids.iter()) {
                owned.extend(Role::ALL.map(|role| Slot::show(&id, role)));
                owned.extend(show.seasons.keys().map(|&number| Slot::season(&id, number)));
            }
        }
        Ok((stored.into_iter())
            .filter(|(slot, _)| owned.contains(slot))
            .collect())
    }

    /// Makes each of `list`'s copies of its image, through the image store. One not kept makes
    /// none.
    async fn copy(&self, list: &[CopyAhead]) {
        for CopyAhead { image, copies, .. } in list {
            match image {
                Image::Downloaded { plugin, path } => {
                    (self.store)
                        .copy_ahead(&artwork::kept_key(plugin, path), copies)
                        .await;
                }
                Image::BesideMedia { library, stored } => {
                    let folder = library.to_string();
                    (self.store)
                        .copy_ahead(&local_images::key(&folder, stored), copies)
                        .await;
                }
                Image::Custom { slot, version } => {
                    let mut key = slot.key();
                    key.push(version);
                    self.store.copy_ahead(&key, copies).await;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "copies_ahead_tests.rs"]
mod tests;
