//! Custom Images: the Posters and Backdrops an admin uploads for a Movie, Show or Season, and the
//! Backdrop for a Library, which everyone sees in place of the downloaded or newest title's ones
//! until they are removed (ADR 0036).
//!
//! [`CustomImages`] owns `custom_images` (see `src/library/AGENTS.md`). It keeps a title's
//! against the title's external id, as Watch State is (ADR 0013), never against an Item, so one
//! outlives its title's files. A Library's goes with the Library. The bytes are in the
//! [`ImageStore`], under a version taken from them.
//!
//! An image is the body of one plain HTTP request (ADR 0007): `PUT /custom-images/<slot>` uploads
//! it, or with `?artwork=<url>` keeps the image the Server offered at that URL (`Artwork::offer`),
//! such as one of a title's artwork list (`Artwork::listed`), downloaded through its title source
//! as if uploaded. Any offered image may go into any slot, not only one it was offered for: the
//! same admin can upload any bytes there anyway, so binding offers to slots would stop nothing.
//! `DELETE` there removes it.
//! `PREFIXES` in `src/api/access.rs` lets only Users with Manage Libraries do either. Anyone with
//! a session fetches it at its [`Urls`] entry, under `/media/`, resized by `?width=` as a
//! downloaded image is (`crate::library::artwork`).

use std::{collections::HashMap, io, iter, sync::Arc};

use axum::{
    Router,
    body::Body,
    extract::{Query, Request, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use serde::Deserialize;
use sqlx::SqliteConnection;

use crate::{
    db::Database,
    images::{Asked, CUSTOM_IMAGES_DIR, ImageStore, Received, Resize, UploadError, percent_decode},
    library::{
        artwork::{Artwork, Error as ArtworkError, poster_grid_copy},
        contents::{library_with_movie, library_with_show, matched_episodes, parse_hint},
    },
    server_settings::ImageSettings,
};

/// Where the store keeps them, under the data directory.
const DIR: &str = CUSTOM_IMAGES_DIR;

/// The Custom Images of titles.
#[derive(Debug, Clone)]
pub(crate) struct CustomImages {
    db: Arc<Database>,
    store: ImageStore,
}

/// Which of a title's images a Custom Image, or an image beside the media, stands in for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Role {
    Poster,
    Backdrop,
}

/// Which image of which title or Library a Custom Image stands in for, as its paths name it:
/// `movie/tmdb-348/poster`, `show/tmdb-1399/backdrop`, `show/tmdb-1399/season/2/poster` or
/// `library/3/backdrop`. A title's id is one segment encoded as `encodeURIComponent` does, since
/// it may hold any character, `/` and `%` among them, and the web builds these paths with it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Slot(String);

/// What a [`Slot`] stands in for an image of, which the Server must have for an upload there.
#[derive(Debug, PartialEq, Eq)]
enum Subject {
    Movie(ExternalId),
    Show(ExternalId),
    Season(ExternalId, i64),
    Library,
}

/// The URL of every Custom Image, by slot: see [`CustomImages::urls`].
#[derive(Debug, Default)]
pub(crate) struct Urls(HashMap<Slot, String>);

/// Why a Custom Image couldn't be kept or removed.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Upload(#[from] UploadError),
    /// The picked artwork couldn't be had from its title source.
    #[error(transparent)]
    Artwork(#[from] ArtworkError),
    /// The Library it would be for isn't there, or was deleted while it arrived.
    #[error("there is no Library {0}")]
    NoLibrary(i64),
    #[error("image store error: {0}")]
    Io(#[from] io::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// The task doing it panicked.
    #[error("the task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl Role {
    /// As paths and the database name it.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Poster => "poster",
            Self::Backdrop => "backdrop",
        }
    }

    /// The role [`Role::name`] gives `name`, if any.
    pub(super) fn named(name: &str) -> Option<Self> {
        match name {
            "poster" => Some(Self::Poster),
            "backdrop" => Some(Self::Backdrop),
            _ => None,
        }
    }
}

impl Slot {
    pub(crate) fn movie(id: &ExternalId, role: Role) -> Self {
        Self(format!("movie/{}/{}", segment(id), role.name()))
    }

    pub(crate) fn show(id: &ExternalId, role: Role) -> Self {
        Self(format!("show/{}/{}", segment(id), role.name()))
    }

    /// A Season has only a Poster.
    pub(crate) fn season(show: &ExternalId, number: i64) -> Self {
        Self(format!("show/{}/season/{number}/poster", segment(show)))
    }

    /// A Library has only a Backdrop.
    pub(crate) fn library(id: i64) -> Self {
        Self(format!("library/{id}/backdrop"))
    }

    /// Whether it is a title's or a Season's Poster.
    fn is_poster(&self) -> bool {
        self.0.ends_with("/poster")
    }

    /// The Library whose image it is, if it is a Library's.
    fn library_id(&self) -> Option<i64> {
        let id = self.0.strip_prefix("library/")?.strip_suffix("/backdrop")?;
        id.parse().ok()
    }

    /// The slot `path` names, and what it stands in for an image of, if it names one, and only as
    /// [`Slot`]'s constructors write it, so each image has one path.
    fn parse(path: &str) -> Option<(Self, Subject)> {
        let title = |segment| parse_hint(&percent_decode(segment)?);
        let parts: Vec<&str> = path.split('/').collect();
        let (slot, subject) = match parts[..] {
            ["movie", id, name] => {
                let id = title(id)?;
                (Self::movie(&id, Role::named(name)?), Subject::Movie(id))
            }
            ["show", id, name] => {
                let id = title(id)?;
                (Self::show(&id, Role::named(name)?), Subject::Show(id))
            }
            ["show", id, "season", number, "poster"] => {
                let (id, number) = (title(id)?, number.parse::<u32>().ok()?.into());
                (Self::season(&id, number), Subject::Season(id, number))
            }
            ["library", id, "backdrop"] => (
                Self::library(id.parse::<u32>().ok()?.into()),
                Subject::Library,
            ),
            _ => return None,
        };
        (slot.0 == path).then_some((slot, subject))
    }

    /// Its folder in the store, which holds one file per version.
    fn key(&self) -> Vec<&str> {
        iter::once(DIR).chain(self.0.split('/')).collect()
    }
}

/// `id`'s text form as one path segment.
fn segment(id: &ExternalId) -> String {
    encode(&id.to_string())
}

/// `text` as `encodeURIComponent` writes it, so a path the web builds is the one [`Slot`] parses
/// back. It keeps the letters, digits and `-` of today's ids, so their paths, which are stored
/// keys, don't change; the store makes each key part a safe file name itself.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => char::from(byte).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

impl Urls {
    /// `urls` by slot, without a database, for a test of what a response shows.
    #[cfg(test)]
    pub(super) fn for_tests(urls: impl IntoIterator<Item = (Slot, String)>) -> Self {
        Self(urls.into_iter().collect())
    }

    /// The URL of the Custom Image at `slot`, if there is one.
    pub(crate) fn of(&self, slot: &Slot) -> Option<String> {
        self.0.get(slot).cloned()
    }

    /// The URL of the Custom Image the title `id` has at `slot(its id)`, by `id` or else by any of
    /// `ids`, in namespace order: any one id identifies a title (ADR 0042), and an admin may have
    /// uploaded it under another.
    pub(crate) fn of_title(
        &self,
        id: &ExternalId,
        ids: &ExternalIds,
        slot: impl Fn(&ExternalId) -> Slot,
    ) -> Option<String> {
        iter::once(id.clone())
            .chain(ids.iter())
            .find_map(|id| self.of(&slot(&id)))
    }
}

impl CustomImages {
    /// The Custom Images recorded in `db`, their bytes kept in `store`.
    pub(crate) fn new(db: Arc<Database>, store: ImageStore) -> Self {
        Self { db, store }
    }

    /// Where clients fetch each Custom Image.
    // ponytail: reads every Custom Image for each response that shows images; there are as many
    // as admins uploaded. Query by slot if that ever grows.
    pub(crate) async fn urls(&self) -> Result<Urls, sqlx::Error> {
        let rows = sqlx::query!("SELECT slot, version FROM custom_images")
            .fetch_all(self.db.reader())
            .await?;
        Ok(Urls(
            rows.into_iter()
                .map(|row| {
                    let url = url(&row.slot, &row.version);
                    (Slot(row.slot), url)
                })
                .collect(),
        ))
    }

    /// The thumbhash of each Custom Image resized so far, by the URL clients fetch it at. A new
    /// version is a new image, so it has none until it is resized.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn thumbhashes(&self) -> sqlx::Result<HashMap<String, Vec<u8>>> {
        let stored = self.store.thumbhashes(DIR).await?;
        Ok(stored
            .into_iter()
            .filter_map(|(key, thumbhash)| {
                let (version, slot) = key.get(1..)?.split_last()?;
                Some((url(&slot.join("/"), version), thumbhash))
            })
            .collect())
    }

    /// Whether the Server has the Movie, Show or Season `subject`. A Library is always had here:
    /// the write that keeps its image checks it.
    async fn server_has(&self, subject: &Subject) -> sqlx::Result<bool> {
        let conn = &mut *self.db.reader().acquire().await?;
        Ok(match subject {
            Subject::Movie(id) => library_with_movie(conn, id).await?.is_some(),
            Subject::Show(id) => library_with_show(conn, id).await?.is_some(),
            Subject::Season(show, number) => {
                !matched_episodes(conn, show, *number).await?.is_empty()
            }
            Subject::Library => true,
        })
    }

    /// Keeps `body` as the Custom Image at `slot`, in place of any there before, then makes
    /// `poster_copy` of it in the background.
    async fn upload(
        &self,
        slot: Slot,
        body: Body,
        poster_copy: Option<Resize>,
    ) -> Result<(), Error> {
        let received = self.store.receive(&slot.key(), body).await?;
        let version = received.version.clone();
        let stored = slot.0.clone();
        let library = slot.library_id();
        let replaced = self
            .db
            .write(move |tx| Box::pin(record(tx, stored, received, library)))
            .await;
        if let Err(Error::NoLibrary(id)) = replaced {
            self.delete_library_dir(id).await?;
        }
        // The same bytes again are the same version, which mustn't go.
        if let Some(old) = replaced?.filter(|old| *old != version) {
            self.delete_file(&slot, &old).await?;
        }
        if let Some(copy) = poster_copy {
            let store = self.store.clone();
            tokio::spawn(async move {
                let mut key = slot.key();
                key.push(&version);
                store.copy_ahead(&key, copy).await;
            });
        }
        Ok(())
    }

    /// Removes the Custom Image at `slot`, if there is one, which brings back the image it stood
    /// in for.
    async fn remove(&self, slot: Slot) -> Result<(), Error> {
        let stored = slot.0.clone();
        let removed = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query_scalar!(
                        "DELETE FROM custom_images WHERE slot = ? RETURNING version",
                        stored
                    )
                    .fetch_optional(&mut **tx)
                    .await
                })
            })
            .await?;
        match removed {
            Some(version) => self.delete_file(&slot, &version).await,
            None => Ok(()),
        }
    }

    /// Deletes the Custom Image of the Library `id`, which was deleted, logging a failure.
    pub(crate) async fn delete_library(&self, id: i64) {
        let deleted = match self.remove(Slot::library(id)).await {
            Ok(()) => self.delete_library_dir(id).await.map_err(Error::from),
            Err(err) => Err(err),
        };
        if let Err(err) = deleted {
            tracing::error!("could not delete the Custom Image of the deleted Library {id}: {err}");
        }
    }

    /// Deletes the Library `id`'s folder in the store, and so any version an upload racing the
    /// Library's delete wrote.
    async fn delete_library_dir(&self, id: i64) -> io::Result<()> {
        self.store
            .delete_dir(&[DIR, "library", &id.to_string()])
            .await
    }

    async fn delete_file(&self, slot: &Slot, version: &str) -> Result<(), Error> {
        let mut key = slot.key();
        key.push(version);
        Ok(self.store.delete(&key).await?)
    }

    /// The Custom Image at `slot` as the answer to `request`, resized as `resize` asks, 404 unless
    /// `version` is its current one, since a browser keeps what it fetched at that URL for good.
    async fn serve(
        &self,
        (slot, version): (&Slot, &str),
        resize: Option<Resize>,
        request: Request,
    ) -> Result<Response, Error> {
        let current = sqlx::query!(
            "SELECT version, content_type FROM custom_images WHERE slot = ?",
            slot.0
        )
        .fetch_optional(self.db.reader())
        .await?;
        let Some(current) = current.filter(|current| current.version == version) else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        let content_type = served_type(&current.content_type);
        let mut key = slot.key();
        key.push(version);
        Ok(self
            .store
            .serve(&key, content_type, resize, request)
            .await?)
    }
}

/// The content type to serve an image stored as `content_type` with, one the store told from its
/// bytes.
pub(super) fn served_type(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "image/png",
        "image/webp" => "image/webp",
        _ => "image/jpeg",
    }
}

/// Records `received` as the Custom Image at the slot `stored`, which is the Library `library`'s
/// if it is one's, and returns the version it replaced, if any. [`Error::NoLibrary`] if that
/// Library isn't there.
async fn record(
    conn: &mut SqliteConnection,
    stored: String,
    received: Received,
    library: Option<i64>,
) -> Result<Option<String>, Error> {
    if let Some(id) = library
        && !library_exists(conn, id).await?
    {
        return Err(Error::NoLibrary(id));
    }
    let replaced = sqlx::query_scalar!("SELECT version FROM custom_images WHERE slot = ?", stored)
        .fetch_optional(&mut *conn)
        .await?;
    sqlx::query!(
        "INSERT INTO custom_images (slot, version, content_type) VALUES (?, ?, ?)
         ON CONFLICT (slot) DO UPDATE
         SET version = excluded.version, content_type = excluded.content_type",
        stored,
        received.version,
        received.content_type,
    )
    .execute(conn)
    .await?;
    Ok(replaced)
}

/// Whether the Library `id` is there. Read in the write that keeps its image, so a Library deleted
/// before that commits refuses the image, and one deleted after deletes it
/// ([`CustomImages::delete_library`]).
async fn library_exists(conn: &mut SqliteConnection, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM libraries WHERE id = ?) AS "found!: bool""#,
        id
    )
    .fetch_one(conn)
    .await
}

/// Where clients fetch the version `version` of the Custom Image at the slot named `slot`.
fn url(slot: &str, version: &str) -> String {
    format!("/media/custom-images/{slot}/{version}")
}

/// Serves uploads to and removals from `/custom-images/<slot>`, picks from `artwork`'s offered
/// images, and the images under `/media/custom-images/<slot>/<version>`, resized as `images` says.
/// Who may make each request is up to the layers the router is put behind.
pub(crate) fn router(
    custom_images: CustomImages,
    artwork: Artwork,
    images: ImageSettings,
) -> Router {
    Router::new()
        .route(&format!("{UPLOADS}{{*slot}}"), put(upload).delete(remove))
        .route(&format!("{IMAGES}{{*path}}"), get(serve))
        .with_state((custom_images, artwork, images))
}

/// What the router's handlers share.
type Handles = (CustomImages, Artwork, ImageSettings);

/// An upload's query: the URL of an offered image to keep in place of the body.
#[derive(Deserialize)]
struct UploadQuery {
    artwork: Option<String>,
}

const UPLOADS: &str = "/custom-images/";
const IMAGES: &str = "/media/custom-images/";

/// The slot path `uri` names after `prefix`, still percent-encoded as [`Slot`] writes it. Axum's
/// `Path` would decode it, which makes an id's encoded `/` a separator.
fn raw_path<'a>(uri: &'a Uri, prefix: &str) -> &'a str {
    uri.path().strip_prefix(prefix).unwrap_or_default()
}

async fn upload(
    State((custom_images, artwork, images)): State<Handles>,
    uri: Uri,
    Query(picked): Query<UploadQuery>,
    body: Body,
) -> Response {
    let Some((slot, subject)) = Slot::parse(raw_path(&uri, UPLOADS)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Before the body is read, so a refusal costs no upload, and not in the write: a title that
    // leaves before it commits keeps its image, as one leaving after would (ADR 0036).
    match custom_images.server_has(&subject).await {
        Ok(true) => {}
        Ok(false) => {
            return (StatusCode::NOT_FOUND, "there is no such title or Season").into_response();
        }
        Err(err) => return failed("keep", &err.into()),
    }
    let poster_copy = poster_grid_copy(&images).filter(|_| slot.is_poster());
    // The write commits even if the client goes away, so deleting what it replaced must follow
    // (ADR 0029).
    let uploaded = tokio::spawn(async move {
        let body = match picked.artwork {
            Some(url) => Body::from(artwork.offered_bytes(&url).await?),
            None => body,
        };
        custom_images.upload(slot, body, poster_copy).await
    });
    match uploaded.await.unwrap_or_else(|err| Err(err.into())) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        // Each refusal has a status of its own, which the web app tells the admin.
        Err(Error::Upload(refused @ UploadError::TooBig)) => {
            (StatusCode::PAYLOAD_TOO_LARGE, refused.to_string()).into_response()
        }
        Err(Error::Upload(refused @ UploadError::NotAnImage)) => {
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, refused.to_string()).into_response()
        }
        Err(Error::Upload(refused @ UploadError::TooManyPixels)) => {
            (StatusCode::UNPROCESSABLE_ENTITY, refused.to_string()).into_response()
        }
        Err(Error::Upload(broke_off @ UploadError::BrokeOff(_))) => {
            (StatusCode::BAD_REQUEST, broke_off.to_string()).into_response()
        }
        Err(missing @ (Error::NoLibrary(_) | Error::Artwork(ArtworkError::NotFound))) => {
            (StatusCode::NOT_FOUND, missing.to_string()).into_response()
        }
        Err(Error::Artwork(unavailable @ ArtworkError::Unavailable(_))) => {
            tracing::warn!("could not pick a Custom Image: {unavailable}");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(err) => failed("keep", &err),
    }
}

async fn remove(State((custom_images, ..)): State<Handles>, uri: Uri) -> Response {
    let Some((slot, _)) = Slot::parse(raw_path(&uri, UPLOADS)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // As for an upload, deleting the file follows the committed write (ADR 0029).
    let removed = tokio::spawn(async move { custom_images.remove(slot).await });
    match removed.await.unwrap_or_else(|err| Err(err.into())) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => failed("remove", &err),
    }
}

async fn serve(
    State((custom_images, _, images)): State<Handles>,
    Query(asked): Query<Asked>,
    request: Request,
) -> Response {
    let uri = request.uri().clone();
    let Some((slot, version)) = raw_path(&uri, IMAGES).rsplit_once('/') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some((slot, _)) = Slot::parse(slot) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let resize = match asked.resize(&images) {
        Ok(resize) => resize,
        Err(refused) => return refused.into_response(),
    };
    match custom_images.serve((&slot, version), resize, request).await {
        Ok(response) => response,
        Err(err) => failed("serve", &err),
    }
}

/// The response to a server-side failure to `what` a Custom Image, whose cause is logged.
fn failed(what: &str, err: &Error) -> Response {
    tracing::error!("could not {what} a Custom Image: {err}");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

#[cfg(test)]
#[path = "custom_images_tests.rs"]
mod tests;
