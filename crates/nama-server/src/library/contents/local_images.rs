//! The images a scan found beside the Movies' and Episodes' files, which stand in for their Movies',
//! Shows' and Seasons' downloaded Posters and Backdrops: see `crate::library::local_images`, which
//! keeps their copies in the image store.

use std::collections::HashMap;

use nama_plugin_api::metadata::ExternalIds;
use sqlx::SqliteConnection;

use super::{Contents, ids_from_json, ids_sql};
use crate::library::Role;

/// What an image beside a Version stands in for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LocalRole {
    /// An image of the title the Version is listed under: its Movie or its Episode's Show.
    Title(Role),
    /// The Poster of its Episode's Season.
    SeasonPoster,
}

impl LocalRole {
    /// As the database names it.
    const fn name(self) -> &'static str {
        match self {
            Self::Title(role) => role.name(),
            Self::SeasonPoster => "season_poster",
        }
    }

    /// The role [`LocalRole::name`] gives `name`, if any.
    fn named(name: &str) -> Option<Self> {
        match name {
            "season_poster" => Some(Self::SeasonPoster),
            _ => Role::named(name).map(Self::Title),
        }
    }
}

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
    pub(crate) role: LocalRole,
    pub(crate) file: LocalImageFile,
    /// The version the image store took from its bytes.
    pub(crate) stored: String,
    pub(crate) content_type: String,
}

/// A Matched title's image beside one of its Versions: see [`Contents::titles_local_images`].
#[derive(Debug)]
pub(crate) struct TitleLocalImage {
    /// Every id the Version's Match gives the Movie, or the Episode's Show.
    pub(crate) ids: ExternalIds,
    pub(crate) role: LocalRole,
    /// The Season of the Episode the Version holds, whose Show's or Season's the image is; `None`
    /// for a Movie's.
    pub(crate) season: Option<i64>,
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
                    role: LocalRole::named(&row.role)?,
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

    /// The Season of the Episodes each of the Library `library`'s Versions holds, by Version id,
    /// the lowest if they span several; a Version holding no Episode is left out. Read from the
    /// stored Items, so an Episode a Recogniser placed counts as a parsed one does.
    pub(crate) async fn episode_seasons(&self, library: i64) -> sqlx::Result<HashMap<i64, i64>> {
        let rows = sqlx::query!(
            r#"SELECT library_items.file_id AS "version!", MIN(library_items.season) AS "season!: i64"
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ? AND library_items.kind = 'episode'
               GROUP BY library_items.file_id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| (row.version, row.season))
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

    /// Every Matched Movie's and Show's images beside its Versions, those of the first-added Item
    /// first: by when it was added, then by its id (#732).
    pub(crate) async fn titles_local_images(&self) -> sqlx::Result<Vec<TitleLocalImage>> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_files.library_id AS "library!", local_images.role,
                              local_images.stored, library_items.season, "#
                        + $ids
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show')) AS "ids?: String"
                       FROM local_images
                       JOIN library_files ON library_files.id = local_images.version_id
                       JOIN library_items ON library_items.file_id = local_images.version_id
                       JOIN item_matches ON item_matches.item_id = library_items.id
                       WHERE item_matches.state = 'matched'
                       ORDER BY library_items.added_at, library_items.id"#
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        Ok((rows.into_iter())
            .filter_map(|row| {
                Some(TitleLocalImage {
                    ids: ids_from_json(row.ids.as_deref()),
                    role: LocalRole::named(&row.role)?,
                    season: row.season,
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
