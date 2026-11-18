//! What a response shows of images in place of the downloaded ones, and where it finds them.

use std::collections::HashMap;

use connectrpc::ConnectError;
use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::database_failed;
use crate::{
    library::{Artwork, CustomImageUrls, CustomImages, LocalImageUrls, LocalImages, Role, Slot},
    server_settings::ImageSettings,
};

/// What a response shows of images: the Custom Images, and else the images beside a title's media,
/// in place of the downloaded ones, and the thumbhash of each image resized so far, by URL.
pub(super) struct ShownImages {
    pub(super) custom: CustomImageUrls,
    pub(super) local: LocalImageUrls,
    pub(super) thumbhashes: HashMap<String, Vec<u8>>,
}

impl ShownImages {
    /// The URL of the image the Movie `id`, with every id `ids`, shows in place of its downloaded
    /// `role`: its Custom Image, or else the image beside its media (`src/library/AGENTS.md`).
    pub(super) fn movie_image(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        role: Role,
    ) -> Option<String> {
        (self.custom.of_title(id, ids, |id| Slot::movie(id, role)))
            .or_else(|| self.local.of_movie(id, ids, role))
    }

    /// As [`ShownImages::movie_image`], for the Show `id`.
    pub(super) fn show_image(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        role: Role,
    ) -> Option<String> {
        (self.custom.of_title(id, ids, |id| Slot::show(id, role)))
            .or_else(|| self.local.of_show(id, ids, role))
    }

    /// The thumbhash to send beside `url`, if the image there has one.
    pub(super) fn thumbhash(&self, url: Option<&str>) -> Option<Vec<u8>> {
        self.thumbhashes.get(url?).cloned()
    }
}

/// Where a response finds the images it shows: see [`ImageSources::shown`].
#[derive(Clone)]
pub(super) struct ImageSources {
    pub(super) custom_images: CustomImages,
    pub(super) local_images: LocalImages,
    pub(super) artwork: Artwork,
    pub(super) settings: ImageSettings,
}

impl ImageSources {
    /// What a response shows of images, with no thumbhashes while resizing is off (ADR 0039).
    pub(super) async fn shown(&self) -> Result<ShownImages, ConnectError> {
        let custom = self.custom_images.urls().await.map_err(database_failed)?;
        let local = self.local_images.urls().await.map_err(database_failed)?;
        let mut thumbhashes = HashMap::new();
        if self.settings.resizing().is_some() {
            let artwork = self.artwork.thumbhashes().await.map_err(database_failed)?;
            let custom_images = self.custom_images.thumbhashes().await;
            let local_images = self.local_images.thumbhashes().await;
            thumbhashes.extend(artwork);
            thumbhashes.extend(custom_images.map_err(database_failed)?);
            thumbhashes.extend(local_images.map_err(database_failed)?);
        }
        Ok(ShownImages {
            custom,
            local,
            thumbhashes,
        })
    }
}
