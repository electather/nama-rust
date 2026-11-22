//! Images beside the media: a Poster or Backdrop that Jellyfin, Kodi and the like leave beside a
//! Movie's file or in a Show's or Season's folder, such as `poster.jpg` or `fanart.jpg`, which
//! stands in for the downloaded one. A Custom Image still goes first: unlike in Jellyfin, a file
//! beside the media never replaces an admin's choice (#732, #1016).
//!
//! A scan finds them, and [`LocalImages`] keeps a copy of each in the [`ImageStore`], under
//! `local-images/<library>/<version>`, if it is what a Custom Image may be; one that isn't is
//! skipped, and the scan logs it. The Server only reads the files, never writes or deletes them.
//! The rows are in `local_images`, which [`Contents`] owns, each recorded beside a Version: an
//! Episode's for its Show's and Season's images. Of a title's Items, the image beside the
//! first-added one shows: adding another Item of a Movie, or Episode of a Show, never changes its
//! Poster.
//!
//! Clients fetch them at their [`LocalImageUrls`] entry, under `/media/`, resized by `?width=` as
//! a downloaded image is, served to anyone with a session.

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    io, iter,
    sync::Arc,
};

use axum::{
    Router,
    body::Body,
    extract::{Path as UrlPath, Query, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use tokio_util::io::ReaderStream;

use crate::{
    db::Database,
    images::{Asked, ImageStore, LOCAL_IMAGES_DIR, UploadError},
    library::{
        Role,
        contents::{Contents, LocalImage, LocalImageFile, LocalRole, TitleLocalImage},
        custom_images::served_type,
    },
    server_settings::ImageSettings,
};

/// Where the store keeps the copies, one folder per Library.
const DIR: &str = LOCAL_IMAGES_DIR;

/// What an image beside the media may stand in for.
const ROLES: [LocalRole; 3] = [
    LocalRole::Title(Role::Poster),
    LocalRole::Title(Role::Backdrop),
    LocalRole::SeasonPoster,
];

/// The images beside the media, copied into the store.
#[derive(Debug, Clone)]
pub(crate) struct LocalImages {
    contents: Contents,
    store: ImageStore,
}

/// The images found beside a Library's Versions, by Version id and what each stands in for.
pub(crate) type FoundImages = HashMap<(i64, LocalRole), LocalImageFile>;

/// The URL of each Matched title's image beside its first-added Item that has one, by each of the
/// title's ids: see [`LocalImages::urls`]. Movies, Shows and Seasons apart, since a Show and a
/// Movie can share an id such as `tmdb-1`.
#[derive(Debug, Default)]
pub(crate) struct LocalImageUrls {
    movies: HashMap<(ExternalId, Role), String>,
    shows: HashMap<(ExternalId, Role), String>,
    seasons: HashMap<(ExternalId, i64), String>,
}

impl LocalImageUrls {
    /// The URL of the image beside the Movie `id`'s media that stands in for its `role`, by `id`
    /// or else by any of `ids`: any one id identifies a title (ADR 0042).
    pub(crate) fn of_movie(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        role: Role,
    ) -> Option<String> {
        first_of(&self.movies, id, ids, |id| (id, role))
    }

    /// As [`LocalImageUrls::of_movie`], for the Show `id`.
    pub(crate) fn of_show(&self, id: &ExternalId, ids: &ExternalIds, role: Role) -> Option<String> {
        first_of(&self.shows, id, ids, |id| (id, role))
    }

    /// As [`LocalImageUrls::of_movie`], for the Poster of the Show `id`'s Season `number`.
    pub(crate) fn of_season(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        number: i64,
    ) -> Option<String> {
        first_of(&self.seasons, id, ids, |id| (id, number))
    }

    /// Files each of `images` under its title's ids; of one title's, the first is kept.
    pub(super) fn filed(images: Vec<TitleLocalImage>) -> Self {
        let mut urls = Self::default();
        for image in images {
            let (ids, url) = (&image.ids, url(image.library, &image.stored));
            match (image.role, image.season) {
                (LocalRole::Title(role), None) => {
                    insert_first(&mut urls.movies, ids, |id| (id, role), &url);
                }
                (LocalRole::Title(role), Some(_)) => {
                    insert_first(&mut urls.shows, ids, |id| (id, role), &url);
                }
                (LocalRole::SeasonPoster, Some(number)) => {
                    insert_first(&mut urls.seasons, ids, |id| (id, number), &url);
                }
                (LocalRole::SeasonPoster, None) => {}
            }
        }
        urls
    }
}

/// Inserts `url` in `map` under each of `ids`, as `key` makes it a key, unless an earlier one is
/// there.
fn insert_first<K: Eq + Hash>(
    map: &mut HashMap<K, String>,
    ids: &ExternalIds,
    key: impl Fn(ExternalId) -> K,
    url: &str,
) {
    for id in ids.iter() {
        map.entry(key(id)).or_insert_with(|| url.to_owned());
    }
}

/// The value `map` has under the key `key` makes of `id`, or else of the first of `ids` it has one
/// under.
fn first_of<K: Eq + Hash>(
    map: &HashMap<K, String>,
    id: &ExternalId,
    ids: &ExternalIds,
    key: impl Fn(ExternalId) -> K,
) -> Option<String> {
    iter::once(id.clone())
        .chain(ids.iter())
        .find_map(|id| map.get(&key(id)).cloned())
}

impl LocalImages {
    /// The images beside the media recorded in `db`, their copies kept in `store`.
    pub(crate) fn new(db: Arc<Database>, store: ImageStore) -> Self {
        Self {
            contents: Contents::new(db),
            store,
        }
    }

    /// Of `read`, the Library `library`'s Versions whose images `found` beside them differ from
    /// those stored, or whose stored copy is gone from the store: the ones [`LocalImages::keep`]
    /// should replace.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn changed(
        &self,
        library: i64,
        read: Vec<i64>,
        found: &FoundImages,
    ) -> sqlx::Result<Vec<i64>> {
        let folder = library.to_string();
        let mut stored = FoundImages::new();
        let mut copy_gone = HashSet::new();
        for image in self.contents.local_images(library).await? {
            // One the store can't tell about counts as gone: copying it again logs why if it still
            // fails, and one no longer beside the media falls back to the downloaded image.
            let key = [DIR, &folder, &image.stored];
            if self.store.contains(&key).await.unwrap_or(false) {
                stored.insert((image.version, image.role), image.file);
            } else {
                copy_gone.insert(image.version);
            }
        }
        let differs = |version: &i64| {
            copy_gone.contains(version)
                || (ROLES.iter())
                    .any(|&role| found.get(&(*version, role)) != stored.get(&(*version, role)))
        };
        Ok(read.into_iter().filter(differs).collect())
    }

    /// Replaces the images stored beside each of `versions`, in the Library `library`, with those
    /// `found` beside it, copying each into the store. One a Custom Image couldn't be is skipped,
    /// and logged.
    ///
    /// # Errors
    ///
    /// If the database can't be written.
    pub(crate) async fn keep(
        &self,
        library: i64,
        versions: &[i64],
        found: &FoundImages,
    ) -> sqlx::Result<()> {
        let mut kept = Vec::new();
        // ponytail: a Show's image is copied once for each Episode Version it stands in for; copy
        // each path once per batch if scans of long Shows get slow.
        let beside = |&version| ROLES.map(|role| (version, role));
        for (version, role) in versions.iter().flat_map(beside) {
            let Some(file) = found.get(&(version, role)) else {
                continue;
            };
            match self.copy(library, &file.path).await {
                Ok((stored, content_type)) => kept.push(LocalImage {
                    version,
                    role,
                    file: file.clone(),
                    stored,
                    content_type,
                }),
                Err(err) => tracing::warn!("skipped the image {}: {err}", file.path),
            }
        }
        (self.contents)
            .replace_local_images(versions.to_vec(), kept)
            .await
    }

    /// Copies the image at `path` into the Library `library`'s folder in the store, as an upload
    /// of a Custom Image would be, and returns its version and content type.
    async fn copy(&self, library: i64, path: &str) -> Result<(String, String), UploadError> {
        let file = tokio::fs::File::open(path).await?;
        let body = Body::from_stream(ReaderStream::new(file));
        let library = library.to_string();
        let received = self.store.receive(&[DIR, &library], body).await?;
        Ok((received.version, received.content_type.to_owned()))
    }

    /// Deletes each copy in the Library `library`'s folder that no image beside its media has any
    /// more, logging a failure. Only under the Library's scan lock, so no scan of it is copying
    /// one it hasn't recorded yet.
    pub(crate) async fn drop_unused(&self, library: i64) {
        if let Err(err) = self.try_drop_unused(library).await {
            tracing::error!(
                "could not delete the unused images beside Library {library}'s media: {err}"
            );
        }
    }

    async fn try_drop_unused(
        &self,
        library: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let used: HashSet<String> = (self.contents.local_images(library).await?.into_iter())
            .map(|image| image.stored)
            .collect();
        let library = library.to_string();
        // ponytail: lists every Library's copies on each scan; list the one folder if that gets
        // slow on a big store.
        for key in self.store.list(DIR).await? {
            if let [_, of, stored] = key.as_slice()
                && *of == library
                && !used.contains(stored)
            {
                self.store.delete(&[DIR, of, stored]).await?;
            }
        }
        Ok(())
    }

    /// Deletes the copies of the deleted Library `id`'s images, logging a failure.
    pub(crate) async fn delete_library(&self, id: i64) {
        if let Err(err) = self.store.delete_dir(&[DIR, &id.to_string()]).await {
            tracing::error!(
                "could not delete the images beside the deleted Library {id}'s media: {err}"
            );
        }
    }

    /// Where clients fetch each Matched title's image beside its first-added Item that has one.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    // ponytail: reads every image beside the media for each response that shows images, as
    // `CustomImages::urls` does. Query by title if that ever grows.
    pub(crate) async fn urls(&self) -> sqlx::Result<LocalImageUrls> {
        let images = self.contents.titles_local_images().await?;
        Ok(LocalImageUrls::filed(images))
    }

    /// The thumbhash of each copy resized so far, by the URL clients fetch it at.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn thumbhashes(&self) -> sqlx::Result<HashMap<String, Vec<u8>>> {
        let resized = self.store.thumbhashes(DIR).await?;
        Ok((resized.into_iter())
            .filter_map(|(key, thumbhash)| {
                let [_, library, stored] = key.as_slice() else {
                    return None;
                };
                Some((url(library.parse().ok()?, stored), thumbhash))
            })
            .collect())
    }

    /// The copy stored as `stored` in the Library `library`'s folder as the answer to `request`,
    /// 404 unless an image beside its media has it.
    async fn serve(
        &self,
        (library, stored): (i64, &str),
        asked: Asked,
        images: &ImageSettings,
        request: Request,
    ) -> Result<Response, io::Error> {
        let resize = match asked.resize(images) {
            Ok(resize) => resize,
            Err(refused) => return Ok(refused.into_response()),
        };
        let content_type =
            (self.contents.local_image_type(library, stored).await).map_err(io::Error::other)?;
        let Some(content_type) = content_type else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        let key = [DIR, &library.to_string(), stored];
        let content_type = served_type(&content_type);
        self.store.serve(&key, content_type, resize, request).await
    }
}

/// Where clients fetch the copy stored as `stored` in the Library `library`'s folder.
fn url(library: i64, stored: &str) -> String {
    format!("/media/local-images/{library}/{stored}")
}

/// Serves the copies at their [`url`]s, resized as `images` says. Whether the caller has a session
/// is up to the layers the router is put behind.
pub(crate) fn router(local_images: LocalImages, images: ImageSettings) -> Router {
    Router::new()
        .route("/media/local-images/{library}/{stored}", get(serve))
        .with_state((local_images, images))
}

async fn serve(
    State((local_images, images)): State<(LocalImages, ImageSettings)>,
    UrlPath((library, stored)): UrlPath<(i64, String)>,
    Query(asked): Query<Asked>,
    request: Request,
) -> Response {
    let served = local_images.serve((library, &stored), asked, &images, request);
    served.await.unwrap_or_else(|err| {
        tracing::error!("could not serve an image beside the media: {err}");
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    })
}

#[cfg(test)]
#[path = "local_images_tests.rs"]
mod tests;
