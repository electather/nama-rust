//! Subtitle Appearance: how big a User's text subtitles are and what's behind them, on every
//! Device. Each part is its default until they pick another, so only what a User picked is
//! stored, in `subtitle_appearances` (`AGENTS.md`).

use buffa::Enumeration;
use nama_proto::nama::v1::{SubtitleBackground, SubtitleSize};

use crate::db::Database;

/// A User's Subtitle Appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SubtitleAppearance {
    pub(crate) size: SubtitleSize,
    pub(crate) background: SubtitleBackground,
}

/// The Subtitle Appearance of `user`: the default size and an outline for what they never
/// picked, or picked as a value this Server no longer knows.
pub(crate) async fn of(db: &Database, user: i64) -> Result<SubtitleAppearance, sqlx::Error> {
    let stored = sqlx::query!(
        "SELECT size, background FROM subtitle_appearances WHERE user_id = ?",
        user
    )
    .fetch_optional(db.reader())
    .await?;
    let (size, background) = stored.map_or((None, None), |row| (row.size, row.background));
    Ok(SubtitleAppearance {
        size: known(size).unwrap_or(SubtitleSize::Default),
        background: known(background).unwrap_or(SubtitleBackground::Outline),
    })
}

/// Sets the size of `user`'s text subtitles, keeping their background.
pub(crate) async fn set_size(
    db: &Database,
    user: i64,
    size: SubtitleSize,
) -> Result<(), sqlx::Error> {
    set(db, user, Some(size as i32), None).await
}

/// Sets what's behind `user`'s text subtitles, keeping their size.
pub(crate) async fn set_background(
    db: &Database,
    user: i64,
    background: SubtitleBackground,
) -> Result<(), sqlx::Error> {
    set(db, user, None, Some(background as i32)).await
}

/// Stores whichever of `size` and `background` is given, leaving the other as it was.
async fn set(
    db: &Database,
    user: i64,
    size: Option<i32>,
    background: Option<i32>,
) -> Result<(), sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO subtitle_appearances (user_id, size, background) VALUES (?, ?, ?)
                 ON CONFLICT (user_id) DO UPDATE SET
                     size = COALESCE(excluded.size, size),
                     background = COALESCE(excluded.background, background)",
                user,
                size,
                background
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// The value a column holds as `stored`, or `None` for none or a number this Server doesn't know.
fn known<E: Enumeration>(stored: Option<i64>) -> Option<E> {
    E::from_i32(i32::try_from(stored?).ok()?)
}
