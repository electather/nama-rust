//! Which image a Movie, Show, Season or Library shows: its Custom Image, else the image beside its
//! media, else the downloaded one (ADR 0036, `src/library/AGENTS.md`). Every response that shows
//! one asks [`ShownImages`] and only maps its [`ShownImage`] to proto.

use std::collections::HashMap;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use crate::{
    library::{
        Role, Slot,
        artwork::{self, Artwork},
        contents::{ImageRef, NewestBackdrop},
        custom_images::{CustomImages, Urls as CustomImageUrls},
        libraries::{Kind as LibraryKind, Library},
        local_images::{LocalImageUrls, LocalImages},
    },
    server_settings::ImageSettings,
};

/// Where a response finds the images it shows: see [`ImageSources::shown`].
#[derive(Clone)]
pub(crate) struct ImageSources {
    pub(crate) custom_images: CustomImages,
    pub(crate) local_images: LocalImages,
    pub(crate) artwork: Artwork,
    pub(crate) settings: ImageSettings,
}

/// What one response shows of images, read once for it: ask it which image each title shows.
#[derive(Debug)]
pub(crate) struct ShownImages {
    custom: CustomImageUrls,
    local: LocalImageUrls,
    /// The thumbhash of each image resized so far, by URL.
    thumbhashes: HashMap<String, Vec<u8>>,
    /// Offers a downloaded image no Match stores, once it is the one shown.
    artwork: Artwork,
}

/// The image a title or Library shows in one role.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ShownImage {
    /// Where clients fetch it; none if there is no image to show.
    pub(crate) url: Option<String>,
    /// Its thumbhash, once it is resized.
    pub(crate) thumbhash: Option<Vec<u8>>,
    /// Whether it is the title's, or the Library's, own Custom Image.
    pub(crate) custom: bool,
}

/// Which image of a title is asked for.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TitleImage {
    Movie(Role),
    Show(Role),
    /// The Poster of the Show's Season with this number.
    SeasonPoster(i64),
}

/// The downloaded image a title shows when it has no Custom Image or image beside its media. Only
/// the caller knows where its path came from, so it says which.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Downloaded<'a> {
    /// One some Match stores, which clients may fetch already.
    Stored { plugin: &'a str, path: &'a str },
    /// One a title source answered with that no Match stores, offered to clients only once it is
    /// shown ("Clients name ids" in `crates/nama-server/AGENTS.md`). Offered on each read, never
    /// stored, so it loads after a restart forgets the offers.
    Offered { plugin: &'a PluginId, path: &'a str },
}

impl<'a> Downloaded<'a> {
    /// `image`, which some Match stores.
    pub(crate) fn stored(image: &'a ImageRef) -> Self {
        Self::Stored {
            plugin: image.plugin.as_str(),
            path: &image.path,
        }
    }

    /// `image`, which a title source answered with and no Match stores.
    pub(crate) const fn offered(image: &'a ImageRef) -> Self {
        Self::Offered {
            plugin: &image.plugin,
            path: image.path.as_str(),
        }
    }
}

impl ImageSources {
    /// What one response shows of images, with no thumbhashes while resizing is off (ADR 0039).
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn shown(&self) -> sqlx::Result<ShownImages> {
        let custom = self.custom_images.urls().await?;
        let local = self.local_images.urls().await?;
        let mut thumbhashes = HashMap::new();
        if self.settings.resizing().is_some() {
            thumbhashes.extend(self.artwork.thumbhashes().await?);
            thumbhashes.extend(self.custom_images.thumbhashes().await?);
            thumbhashes.extend(self.local_images.thumbhashes().await?);
        }
        Ok(ShownImages {
            custom,
            local,
            thumbhashes,
            artwork: self.artwork.clone(),
        })
    }
}

impl ShownImages {
    /// The image the title `id`, with every id `ids`, shows as its `image`: its Custom Image kept
    /// under any of the ids (ADR 0042), else the image beside its media, else `downloaded`.
    pub(crate) fn of_title(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        image: TitleImage,
        downloaded: Option<Downloaded<'_>>,
    ) -> ShownImage {
        if let Some(url) = self.custom.of_title(id, ids, |id| image.slot(id)) {
            return self.with_thumbhash(url, true);
        }
        match self.beside_media(id, ids, image) {
            Some(url) => self.with_thumbhash(url, false),
            None => self.downloaded(downloaded),
        }
    }

    /// The image `library` shows as its Backdrop: its own Custom Image, else the Backdrop its
    /// newest title with a downloaded one, `newest`, shows. Only the Library's own is `custom`.
    pub(crate) fn of_library(
        &self,
        library: &Library,
        newest: Option<&NewestBackdrop>,
    ) -> ShownImage {
        if let Some(url) = self.custom.of(&Slot::library(library.id)) {
            return self.with_thumbhash(url, true);
        }
        let Some(newest) = newest else {
            return ShownImage::default();
        };
        let image = match library.kind {
            LibraryKind::Movies => TitleImage::Movie(Role::Backdrop),
            LibraryKind::Shows => TitleImage::Show(Role::Backdrop),
        };
        let downloaded = Downloaded::Stored {
            plugin: &newest.plugin,
            path: &newest.path,
        };
        ShownImage {
            custom: false,
            ..self.of_title(&newest.title, &newest.ids, image, Some(downloaded))
        }
    }

    /// The downloaded image, for those that have no Custom Image or image beside the media: the
    /// titled Backdrop, the Title Logo, an Episode's Still (#1339) and a Role's headshot.
    pub(crate) fn downloaded(&self, downloaded: Option<Downloaded<'_>>) -> ShownImage {
        let url = downloaded.map(|downloaded| match downloaded {
            Downloaded::Stored { plugin, path } => artwork::url(plugin, path),
            Downloaded::Offered { plugin, path } => self.artwork.offer(plugin, path),
        });
        ShownImage {
            thumbhash: self.thumbhash(url.as_deref()),
            url,
            custom: false,
        }
    }

    fn beside_media(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        image: TitleImage,
    ) -> Option<String> {
        match image {
            TitleImage::Movie(role) => self.local.of_movie(id, ids, role),
            TitleImage::Show(role) => self.local.of_show(id, ids, role),
            TitleImage::SeasonPoster(number) => self.local.of_season(id, ids, number),
        }
    }

    fn with_thumbhash(&self, url: String, custom: bool) -> ShownImage {
        ShownImage {
            thumbhash: self.thumbhash(Some(&url)),
            url: Some(url),
            custom,
        }
    }

    fn thumbhash(&self, url: Option<&str>) -> Option<Vec<u8>> {
        self.thumbhashes.get(url?).cloned()
    }
}

impl TitleImage {
    fn slot(self, id: &ExternalId) -> Slot {
        match self {
            Self::Movie(role) => Slot::movie(id, role),
            Self::Show(role) => Slot::show(id, role),
            Self::SeasonPoster(number) => Slot::season(id, number),
        }
    }
}

#[cfg(test)]
#[path = "shown_images_tests.rs"]
mod tests;
