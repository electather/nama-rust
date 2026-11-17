//! The images a scan found beside the Movies' files, which stand in for their downloaded Posters and
//! Backdrops: see `crate::library::local_images`, which keeps their copies in the image store.

use nama_plugin_api::metadata::ExternalIds;
use sqlx::SqliteConnection;

use super::{Contents, ids_from_json, ids_sql};
use crate::library::Role;

/// An image file beside a Version, as a scan finds it: all a scan compares with what's stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalImageFile {
    pub(crate) path: String,
    pub(crate) size: i64,
    pub(crate) modified_ns: i64,
}

/// The image beside the Version `version` that stands in for its title's `role`, copied into the
/// image store.
#[derive(Debug)]
pub(crate) struct LocalImage {
    pub(crate) version: i64,
    pub(crate) role: Role,
    pub(crate) file: LocalImageFile,
    /// The version the image store took from its bytes.
    pub(crate) stored: String,
    pub(crate) content_type: String,
}

/// A Matched Movie's image beside one of its Versions: see [`Contents::movies_local_images`].
#[derive(Debug)]
pub(crate) struct MovieLocalImage {
    /// Every id the Version's Match gives the Movie.
    pub(crate) ids: ExternalIds,
    pub(crate) role: Role,
    pub(crate) library: i64,
    pub(crate) stored: String,
}

impl Contents {
    /// The images stored beside the Library `library`'s Versions.
    pub(crate) async fn local_images(&self, library: i64) -> sqlx::Result<Vec<LocalImage>> {
        let rows = sqlx::query!(
            "SELECT local_images.version_id, local_images.role, local_images.path,
                    local_images.size, local_images.modified_ns, local_images.stored,
                    local_images.content_type
             FROM local_images
             JOIN library_files ON library_files.id = local_images.version_id
             WHERE library_files.library_id = ?",
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .filter_map(|row| {
                Some(LocalImage {
                    version: row.version_id,
                    role: Role::named(&row.role)?,
                    file: LocalImageFile {
                        path: row.path,
                        size: row.size,
                        modified_ns: row.modified_ns,
                    },
                    stored: row.stored,
                    content_type: row.content_type,
                })
            })
            .collect())
    }

    /// Replaces the images stored beside each of `versions` with those of `kept` beside it, in one
    /// write.
    pub(crate) async fn replace_local_images(
        &self,
        versions: Vec<i64>,
        kept: Vec<LocalImage>,
    ) -> sqlx::Result<()> {
        self.db
            .write(move |tx| Box::pin(replace(tx, versions, kept)))
            .await
    }

    /// Every Matched Movie's images beside its Versions, those of the first-added Item first: by
    /// when it was added, then by its id (#732).
    pub(crate) async fn movies_local_images(&self) -> sqlx::Result<Vec<MovieLocalImage>> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_files.library_id AS "library!", local_images.role,
                              local_images.stored, "#
                        + $ids
                        + r#"'title') AS "ids?: String"
                       FROM local_images
                       JOIN library_files ON library_files.id = local_images.version_id
                       JOIN library_items ON library_items.file_id = local_images.version_id
                       JOIN item_matches ON item_matches.item_id = library_items.id
                       WHERE library_items.kind = 'movie' AND item_matches.state = 'matched'
                       ORDER BY library_items.added_at, library_items.id"#
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        Ok((rows.into_iter())
            .filter_map(|row| {
                Some(MovieLocalImage {
                    ids: ids_from_json(row.ids.as_deref()),
                    role: Role::named(&row.role)?,
                    library: row.library,
                    stored: row.stored,
                })
            })
            .collect())
    }

    /// The content type of the image stored as `stored` beside a Version in the Library `library`,
    /// if one is.
    pub(crate) async fn local_image_type(
        &self,
        library: i64,
        stored: &str,
    ) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar!(
            "SELECT local_images.content_type FROM local_images
             JOIN library_files ON library_files.id = local_images.version_id
             WHERE library_files.library_id = ? AND local_images.stored = ?
             LIMIT 1",
            library,
            stored
        )
        .fetch_optional(self.db.reader())
        .await
    }
}

async fn replace(
    conn: &mut SqliteConnection,
    versions: Vec<i64>,
    kept: Vec<LocalImage>,
) -> sqlx::Result<()> {
    let versions = serde_json::Value::from(versions).to_string();
    sqlx::query!(
        "DELETE FROM local_images WHERE version_id IN (SELECT value FROM json_each(?))",
        versions
    )
    .execute(&mut *conn)
    .await?;
    for image in kept {
        let role = image.role.name();
        sqlx::query!(
            "INSERT INTO local_images (version_id, role, path, size, modified_ns, stored,
                                       content_type)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            image.version,
            role,
            image.file.path,
            image.file.size,
            image.file.modified_ns,
            image.stored,
            image.content_type,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
