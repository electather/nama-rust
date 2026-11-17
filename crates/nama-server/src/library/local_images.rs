//! Images beside the media: a Poster or Backdrop that Jellyfin, Kodi and the like leave beside a
//! Movie's file, such as `poster.jpg` or `fanart.jpg`, which stands in for the downloaded one.
//! A Custom Image still goes first: unlike in Jellyfin, a file beside the media never replaces an
//! admin's choice (#732).
//!
//! A scan finds them, and [`LocalImages`] keeps a copy of each in the [`ImageStore`], under
//! `local-images/<library>/<version>`, if it is what a Custom Image may be; one that isn't is
//! skipped, and the scan logs it. The Server only reads the files, never writes or deletes them.
//! The rows are in `local_images`, which [`Contents`] owns. Of a Movie's Items, the image beside
//! the first-added one shows: adding another copy of a Movie never changes its Poster.
//!
//! Clients fetch them at their [`LocalImageUrls`] entry, under `/media/`, resized by `?width=` as
//! a downloaded image is, served to anyone with a session.

use std::{
    collections::{HashMap, HashSet},
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
        contents::{Contents, LocalImage, LocalImageFile},
        custom_images::served_type,
    },
    server_settings::ImageSettings,
};

/// Where the store keeps the copies, one folder per Library.
const DIR: &str = LOCAL_IMAGES_DIR;

/// The images beside the media, copied into the store.
#[derive(Debug, Clone)]
pub(crate) struct LocalImages {
    contents: Contents,
    store: ImageStore,
}

/// The images found beside a Library's Versions, by Version id and what each stands in for.
pub(crate) type FoundImages = HashMap<(i64, Role), LocalImageFile>;

/// The URL of each Matched Movie's image beside its first-added Item that has one, by each of the
/// Movie's ids and role: see [`LocalImages::urls`].
#[derive(Debug, Default)]
pub(crate) struct LocalImageUrls(HashMap<(ExternalId, Role), String>);

impl LocalImageUrls {
    /// The URL of the image beside the Movie `id`'s media that stands in for its `role`, by `id`
    /// or else by any of `ids`: any one id identifies a title (ADR 0042).
    pub(crate) fn of_movie(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        role: Role,
    ) -> Option<String> {
        iter::once(id.clone())
            .chain(ids.iter())
            .find_map(|id| self.0.get(&(id, role)).cloned())
    }
}

impl LocalImages {
    /// The images beside the media recorded in `db`, their copies kept in `store`.
    pub(crate) fn new(db: Arc<Database>, store: ImageStore) -> Self {
        Self {
            contents: Contents::new(db),
            store,
        }
    }

    /// The images stored beside the Library `library`'s Versions whose copies the store holds, as
    /// a scan found them, so the next scan copies one gone from the store again.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn stored(&self, library: i64) -> sqlx::Result<FoundImages> {
        let mut found = FoundImages::new();
        let folder = library.to_string();
        for image in self.contents.local_images(library).await? {
            // One the store can't tell about is copied again, which logs why if it still fails.
            let key = [DIR, &folder, &image.stored];
            if self.store.contains(&key).await.unwrap_or(false) {
                found.insert((image.version, image.role), image.file);
            }
        }
        Ok(found)
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
        let beside = |&version| [Role::Poster, Role::Backdrop].map(|role| (version, role));
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
        let received = self.store.receive(&[DIR, &library], body, false).await?;
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

    /// Where clients fetch each Matched Movie's image beside its first-added Item that has one.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    // ponytail: reads every image beside the media for each response that shows images, as
    // `CustomImages::urls` does. Query by title if that ever grows.
    pub(crate) async fn urls(&self) -> sqlx::Result<LocalImageUrls> {
        let mut urls = HashMap::new();
        for image in self.contents.movies_local_images().await? {
            let url = url(image.library, &image.stored);
            for id in image.ids.iter() {
                urls.entry((id, image.role)).or_insert_with(|| url.clone());
            }
        }
        Ok(LocalImageUrls(urls))
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
