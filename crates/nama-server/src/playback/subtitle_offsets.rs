//! Subtitle Offsets: how much later than its file times it each User shows each subtitle Track of a
//! Version, on every Device, kept in `subtitle_offsets` (`AGENTS.md`, ADR 0070).

use std::collections::HashMap;

use super::{Track, TrackKind};
use crate::db::Database;

/// One User's Subtitle Offsets for one Version's Tracks.
#[derive(Debug, Default)]
pub(crate) struct SubtitleOffsets(HashMap<String, i64>);

impl SubtitleOffsets {
    /// The offset of `track`, at `place` in the Version's Tracks, in milliseconds: 0 until set.
    pub(crate) fn of(&self, place: usize, track: &Track) -> i32 {
        let stored = self.0.get(&key(place, track)).copied().unwrap_or_default();
        // Only an `i32` is ever stored.
        i32::try_from(stored).unwrap_or_default()
    }
}

/// `user`'s Subtitle Offsets for the Version `version`'s Tracks.
pub(crate) async fn of(
    db: &Database,
    user: i64,
    version: i64,
) -> Result<SubtitleOffsets, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT track, offset_ms FROM subtitle_offsets WHERE user_id = ? AND version_id = ?",
        user,
        version
    )
    .fetch_all(db.reader())
    .await?;
    Ok(SubtitleOffsets(
        rows.into_iter()
            .map(|row| (row.track, row.offset_ms))
            .collect(),
    ))
}

/// Sets `user`'s offset of `track`, a Track with its place in the Version `version`'s Tracks, to
/// `offset_ms`.
pub(crate) async fn set(
    db: &Database,
    user: i64,
    version: i64,
    (place, track): (usize, &Track),
    offset_ms: i32,
) -> Result<(), sqlx::Error> {
    let track = key(place, track);
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO subtitle_offsets (user_id, version_id, track, offset_ms)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT (user_id, version_id, track) DO UPDATE SET
                     offset_ms = excluded.offset_ms",
                user,
                version,
                track,
                offset_ms
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// What names `track`, at `place`, among its Version's Tracks for good: a sidecar's or downloaded
/// subtitle's path, since one added later can shift the places after the file's own, or else its
/// place, which the file's own Tracks keep.
fn key(place: usize, track: &Track) -> String {
    if let TrackKind::Subtitle {
        sidecar: Some(sidecar),
        ..
    } = &track.kind
    {
        sidecar.path.clone()
    } else {
        place.to_string()
    }
}
