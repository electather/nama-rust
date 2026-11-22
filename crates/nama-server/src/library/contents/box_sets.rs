//! Box Sets (`GLOSSARY.md`), kept once per title (ADR 0081): each Movie's in `title_details`
//! beside its Score, and each Box Set's details and Movies in `box_sets`.

use nama_plugin_api::{PluginId, metadata::ExternalId};
use sqlx::SqliteConnection;

use super::{Contents, ImageRef, Metadata, Movie, parse_hint};
use crate::library::title_source::ListedMovie;

/// A Box Set as stored, with every Movie it groups, Matched or not.
#[derive(Debug, Clone)]
pub(crate) struct StoredBoxSet {
    pub(crate) id: ExternalId,
    /// The Plugin whose title source gave it, which serves its images and its missing Movies'.
    pub(crate) plugin: PluginId,
    pub(crate) name: String,
    pub(crate) overview: Option<String>,
    pub(crate) poster: Option<ImageRef>,
    pub(crate) backdrop: Option<ImageRef>,
    /// The Movies its title source lists, in release order.
    pub(crate) listed: Vec<ExternalId>,
    /// The Movies whose details name it, which it groups even when its title source lists none.
    pub(crate) named_by: Vec<ExternalId>,
    /// `listed` as its title source had them when last looked up, which shows those the Server
    /// lacks.
    pub(crate) looked_up: Vec<ListedMovie>,
}

/// Stores the Box Set `matched` gives its Movie, in place of what was stored under its id: the
/// Movie fetched last wins, but keeps the stored lookups of its Movies if Matching couldn't make
/// them.
pub(super) async fn store(
    conn: &mut SqliteConnection,
    matched: &Metadata,
) -> Result<(), sqlx::Error> {
    let Some(box_set) = &matched.box_set else {
        return Ok(());
    };
    let (namespace, value) = (box_set.id.namespace(), box_set.id.value());
    let plugin = matched.plugin.as_str();
    let listed: Vec<String> = box_set.movies.iter().map(ToString::to_string).collect();
    let listed = to_json(&listed)?;
    let looked_up = box_set.looked_up.as_ref().map(to_json).transpose()?;
    sqlx::query!(
        "INSERT OR REPLACE INTO box_sets
             (namespace, value, plugin, name, overview, poster, backdrop, movies, looked_up)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 coalesce(?9, (SELECT looked_up FROM box_sets WHERE namespace = ?1 AND value = ?2),
                          '[]'))",
        namespace,
        value,
        plugin,
        box_set.name,
        box_set.overview,
        box_set.poster,
        box_set.backdrop,
        listed,
        looked_up,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Deletes the Box Sets `plugin`'s title source gave.
pub(super) async fn forget_plugin(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM box_sets WHERE plugin = ?", plugin)
        .execute(conn)
        .await?;
    Ok(())
}

impl Contents {
    /// The id of the Box Set `movie`'s title source last said it belongs to, if any.
    pub(crate) async fn movie_box_set<V>(
        &self,
        movie: &Movie<V>,
    ) -> Result<Option<ExternalId>, sqlx::Error> {
        let Some(id) = movie.ids.iter().next() else {
            return Ok(None);
        };
        let (namespace, value) = (id.namespace(), id.value());
        let box_set = sqlx::query_scalar!(
            "SELECT box_set FROM title_details
             WHERE plugin = ? AND kind = 'movie' AND namespace = ? AND value = ?",
            movie.plugin,
            namespace,
            value,
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(box_set.flatten().as_deref().and_then(parse_hint))
    }

    /// The Box Set `id`, if one is stored.
    pub(crate) async fn box_set(
        &self,
        id: &ExternalId,
    ) -> Result<Option<StoredBoxSet>, sqlx::Error> {
        let (namespace, value) = (id.namespace(), id.value());
        let Some(row) = sqlx::query!(
            "SELECT plugin, name, overview, poster, backdrop, movies, looked_up FROM box_sets
             WHERE namespace = ? AND value = ?",
            namespace,
            value,
        )
        .fetch_optional(self.db.reader())
        .await?
        else {
            return Ok(None);
        };
        let box_set = id.to_string();
        let named_by = sqlx::query!(
            "SELECT namespace, value FROM title_details WHERE kind = 'movie' AND box_set = ?",
            box_set,
        )
        .fetch_all(self.db.reader())
        .await?;
        let listed: Vec<String> = from_json(&row.movies)?;
        let plugin = PluginId::from(row.plugin);
        Ok(Some(StoredBoxSet {
            id: id.clone(),
            name: row.name,
            overview: row.overview,
            poster: ImageRef::of(&plugin, row.poster.as_ref()),
            backdrop: ImageRef::of(&plugin, row.backdrop.as_ref()),
            plugin,
            listed: listed
                .iter()
                .filter_map(|listed| parse_hint(listed))
                .collect(),
            named_by: (named_by.into_iter())
                .filter_map(|named| ExternalId::new(&named.namespace, named.value).ok())
                .collect(),
            looked_up: from_json(&row.looked_up)?,
        }))
    }

    /// The ids of the Box Sets the Movies Matched in the Library `library` belong to, each once.
    pub(crate) async fn box_sets_in(&self, library: i64) -> Result<Vec<ExternalId>, sqlx::Error> {
        let ids = sqlx::query_scalar!(
            r#"SELECT DISTINCT title_details.box_set AS "box_set!"
               FROM item_matches
               JOIN library_items ON library_items.id = item_matches.item_id
               JOIN library_files ON library_files.id = library_items.file_id
               -- Keyed as `details::store` keys it.
               JOIN title_details
                   ON title_details.plugin = item_matches.plugin
                  AND title_details.kind = 'movie'
                  AND (title_details.namespace, title_details.value) =
                      (SELECT keyed.namespace, keyed.value FROM item_match_ids AS keyed
                       WHERE keyed.item_id = item_matches.item_id AND keyed.scope = 'title'
                       ORDER BY keyed.namespace LIMIT 1)
               WHERE library_files.library_id = ?
                 AND item_matches.state = 'matched'
                 AND library_items.kind = 'movie'
                 AND title_details.box_set IS NOT NULL"#,
            library,
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(ids.iter().filter_map(|id| parse_hint(id)).collect())
    }
}

fn to_json(value: &impl serde::Serialize) -> Result<String, sqlx::Error> {
    serde_json::to_string(value).map_err(|err| sqlx::Error::Encode(err.into()))
}

fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, sqlx::Error> {
    serde_json::from_str(json).map_err(|err| sqlx::Error::Decode(err.into()))
}
