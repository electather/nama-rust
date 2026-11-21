//! The title a Library's list sorts a Movie or Show under, stored with each Match as `sort_title`.

use icu_normalizer::DecomposingNormalizerBorrowed;
use icu_properties::{CodePointMapData, props::GeneralCategory};
use sqlx::SqliteConnection;

use super::Contents;

/// How many Matches [`Contents::fill_sort_titles`] fills in one write.
const BATCH: i64 = 500;

/// `title` as lists sort it: lowercase, without accents or a leading English "The", "A" or "An",
/// so *The Matrix* sorts under M and "éclair" beside "Eclair". SQLite here has no ICU collation,
/// and `NOCASE` folds ASCII only (ADR 0055).
pub(super) fn sort_title(title: &str) -> String {
    let folded = folded(title.trim());
    let rest = ["the ", "a ", "an "]
        .iter()
        .find_map(|article| folded.strip_prefix(article));
    rest.map_or_else(|| folded.clone(), |rest| rest.trim_start().to_owned())
}

/// `text` lowercase and without accents.
pub(super) fn folded(text: &str) -> String {
    let marks = CodePointMapData::<GeneralCategory>::new();
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(text)
        .chars()
        .filter(|&c| marks.get(c) != GeneralCategory::NonspacingMark)
        .collect::<String>()
        .to_lowercase()
}

impl Contents {
    /// Stores the sort title of every Match without one, those stored before Matches had them,
    /// a batch per write so scans aren't held up. Startup runs it once, before serving.
    pub(crate) async fn fill_sort_titles(&self) -> Result<(), sqlx::Error> {
        loop {
            let rows = sqlx::query!(
                r#"SELECT item_id AS "item_id!", coalesce(show_title, title) AS "title!: String"
                   FROM item_matches
                   WHERE state = 'matched' AND sort_title IS NULL
                   LIMIT ?"#,
                BATCH
            )
            .fetch_all(self.db.reader())
            .await?;
            if rows.is_empty() {
                return Ok(());
            }
            let sorted: Vec<(i64, String)> = (rows.into_iter())
                .map(|row| (row.item_id, sort_title(&row.title)))
                .collect();
            self.db
                .write(move |tx| Box::pin(store_sort_titles(tx, sorted)))
                .await?;
        }
    }
}

/// Stores each of `sorted`, a Match's Item id and its sort title.
async fn store_sort_titles(
    conn: &mut SqliteConnection,
    sorted: Vec<(i64, String)>,
) -> Result<(), sqlx::Error> {
    for (item, title) in sorted {
        sqlx::query!(
            "UPDATE item_matches SET sort_title = ? WHERE item_id = ?",
            title,
            item
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "sort_titles_tests.rs"]
mod tests;
