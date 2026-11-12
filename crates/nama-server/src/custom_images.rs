//! Custom Images: the Posters and Backdrops an admin uploads for a Movie, Show or Season, and the
//! Backdrop for a Library, which everyone sees in place of the downloaded or newest title's ones
//! until they are removed (ADR 0036).
//!
//! [`CustomImages`] owns `custom_images` (see `AGENTS.md`). It keeps a title's against the title's
//! external id, as Watch State is (ADR 0013), never against an Item, so one outlives its title's
//! files. A Library's goes with the Library. The bytes are in the [`ImageStore`], under a version
//! taken from them.
//!
//! An image is the body of one plain HTTP request (ADR 0007): `PUT /custom-images/<slot>` uploads
//! it and `DELETE` there removes it, which `PREFIXES` in `src/api.rs` lets only Users with Manage
//! Libraries do. Anyone with a session fetches it at its [`Urls`] entry, under `/media/`.

use std::{collections::HashMap, io, iter, path::Path, sync::Arc};

use axum::{
    Router,
    body::Body,
    extract::{Path as UrlPath, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
};
use nama_plugin_api::metadata::ExternalId;
use sqlx::SqliteConnection;

use crate::{
    contents::parse_hint,
    db::Database,
    images::{ImageStore, Received, UploadError},
};

/// Where the store keeps them, under the data directory.
const DIR: &str = "custom-images";

/// The Custom Images of titles.
#[derive(Debug, Clone)]
pub(crate) struct CustomImages {
    db: Arc<Database>,
    store: ImageStore,
}

/// Which of a title's images a Custom Image stands in for.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Role {
    Poster,
    Backdrop,
}

/// Which image of which title or Library a Custom Image stands in for, as its paths name it:
/// `movie/tmdb-348/poster`, `show/tmdb-1399/backdrop`, `show/tmdb-1399/season/2/poster` or
/// `library/3/backdrop`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Slot(String);

/// The URL of every Custom Image, by slot: see [`CustomImages::urls`].
#[derive(Debug, Default)]
pub(crate) struct Urls(HashMap<Slot, String>);

/// Why a Custom Image couldn't be kept or removed.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Upload(#[from] UploadError),
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
    const fn name(self) -> &'static str {
        match self {
            Self::Poster => "poster",
            Self::Backdrop => "backdrop",
        }
    }
}

impl Slot {
    pub(crate) fn movie(id: &ExternalId, role: Role) -> Self {
        Self(format!("movie/{id}/{}", role.name()))
    }

    pub(crate) fn show(id: &ExternalId, role: Role) -> Self {
        Self(format!("show/{id}/{}", role.name()))
    }

    /// A Season has only a Poster.
    pub(crate) fn season(show: &ExternalId, number: i64) -> Self {
        Self(format!("show/{show}/season/{number}/poster"))
    }

    /// A Library has only a Backdrop.
    pub(crate) fn library(id: i64) -> Self {
        Self(format!("library/{id}/backdrop"))
    }

    /// The Library whose image it is, if it is a Library's.
    fn library_id(&self) -> Option<i64> {
        let id = self.0.strip_prefix("library/")?.strip_suffix("/backdrop")?;
        id.parse().ok()
    }

    /// The slot `path` names, if it names one, and only as [`Slot`]'s constructors write it, so
    /// each image has one path.
    fn parse(path: &str) -> Option<Self> {
        let role = |name| match name {
            "poster" => Some(Role::Poster),
            "backdrop" => Some(Role::Backdrop),
            _ => None,
        };
        let parts: Vec<&str> = path.split('/').collect();
        let slot = match parts[..] {
            ["movie", id, name] => Self::movie(&parse_hint(id)?, role(name)?),
            ["show", id, name] => Self::show(&parse_hint(id)?, role(name)?),
            ["show", id, "season", number, "poster"] => {
                Self::season(&parse_hint(id)?, number.parse::<u32>().ok()?.into())
            }
            ["library", id, "backdrop"] => Self::library(id.parse::<u32>().ok()?.into()),
            _ => return None,
        };
        (slot.0 == path).then_some(slot)
    }

    /// Its folder in the store, which holds one file per version.
    fn key(&self) -> Vec<&str> {
        iter::once(DIR).chain(self.0.split('/')).collect()
    }
}

impl Urls {
    /// The URL of the Custom Image at `slot`, if there is one.
    pub(crate) fn of(&self, slot: &Slot) -> Option<String> {
        self.0.get(slot).cloned()
    }
}

impl CustomImages {
    /// The Custom Images recorded in `db`, their bytes stored in `data_dir`.
    pub(crate) fn new(db: Arc<Database>, data_dir: &Path) -> Self {
        Self {
            db,
            store: ImageStore::new(data_dir),
        }
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
                    let url = format!("/media/custom-images/{}/{}", row.slot, row.version);
                    (Slot(row.slot), url)
                })
                .collect(),
        ))
    }

    /// Keeps `body` as the Custom Image at `slot`, in place of any there before.
    async fn upload(&self, slot: Slot, body: Body) -> Result<(), Error> {
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
        let replaced = replaced?;
        // The same bytes again are the same version, which mustn't go.
        match replaced {
            Some(old) if old != version => self.delete_file(&slot, &old).await,
            _ => Ok(()),
        }
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

    /// The Custom Image at `slot` as the answer to `request`, 404 unless `version` is its current
    /// one, since a browser keeps what it fetched at that URL for good.
    async fn serve(&self, slot: &Slot, version: &str, request: Request) -> Result<Response, Error> {
        let current = sqlx::query!(
            "SELECT version, content_type FROM custom_images WHERE slot = ?",
            slot.0
        )
        .fetch_optional(self.db.reader())
        .await?;
        let Some(current) = current.filter(|current| current.version == version) else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        let content_type = match current.content_type.as_str() {
            "image/png" => "image/png",
            "image/webp" => "image/webp",
            _ => "image/jpeg",
        };
        let mut key = slot.key();
        key.push(version);
        Ok(self.store.serve(&key, content_type, request).await?)
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

/// Serves uploads to and removals from `/custom-images/<slot>`, and the images under
/// `/media/custom-images/<slot>/<version>`. Who may make each request is up to the layers the
/// router is put behind.
pub(crate) fn router(custom_images: CustomImages) -> Router {
    Router::new()
        .route("/custom-images/{*slot}", put(upload).delete(remove))
        .route("/media/custom-images/{*path}", get(serve))
        .with_state(custom_images)
}

async fn upload(
    State(custom_images): State<CustomImages>,
    UrlPath(slot): UrlPath<String>,
    body: Body,
) -> Response {
    let Some(slot) = Slot::parse(&slot) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // The write commits even if the client goes away, so deleting what it replaced must follow
    // (ADR 0029).
    let uploaded = tokio::spawn(async move { custom_images.upload(slot, body).await });
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
        Err(missing @ Error::NoLibrary(_)) => {
            (StatusCode::NOT_FOUND, missing.to_string()).into_response()
        }
        Err(err) => failed("keep", &err),
    }
}

async fn remove(
    State(custom_images): State<CustomImages>,
    UrlPath(slot): UrlPath<String>,
) -> Response {
    let Some(slot) = Slot::parse(&slot) else {
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
    State(custom_images): State<CustomImages>,
    UrlPath(path): UrlPath<String>,
    request: Request,
) -> Response {
    let Some((slot, version)) = path.rsplit_once('/') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(slot) = Slot::parse(slot) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match custom_images.serve(&slot, version, request).await {
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
mod tests {
    use super::Slot;

    #[test]
    fn a_slot_is_named_by_one_path_only() {
        for path in [
            "movie/tmdb-348/poster",
            "movie/imdb-tt0078748/backdrop",
            "show/tvdb-81189/backdrop",
            "show/tmdb-1399/season/0/poster",
            "library/3/backdrop",
        ] {
            assert_eq!(Slot::parse(path), Some(Slot(path.to_owned())), "{path}");
        }
        for path in [
            "movie/tmdb-0348/poster",
            "movie/tmdb-348/logo",
            "movie/tmdb-348",
            "episode/tmdb-1/poster",
            "show/tmdb-1399/season/2/backdrop",
            "show/tmdb-1399/season/-1/poster",
            "movie/tmdb-348/poster/",
            "movie/../poster",
            "library/03/backdrop",
            "library/-3/backdrop",
            "library/3/poster",
            "library/tmdb-3/backdrop",
        ] {
            assert_eq!(Slot::parse(path), None, "{path}");
        }
    }
}
