//! Sidecars: the subtitle files beside each Version, and what their names say they hold.
//!
//! [`Sidecars`] owns `sidecar_subtitles` (see the area's `AGENTS.md`). A scan finds the files and
//! says which Version each is beside; their rows go when the Version goes. Only their names and
//! metadata are read, never the files. Readers of a Version's Tracks get them through
//! [`super::StoredTracks`] (see the area's `AGENTS.md`).

use std::{collections::HashMap, ffi::OsStr, path::Path, sync::Arc};

use sqlx::SqliteConnection;

use super::{Probe, Track, TrackKind, track_preferences::language};
use crate::db::Database;

/// A sidecar as a scan finds it, all a scan compares with what's stored, and the file its Track
/// is read from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Sidecar {
    pub(crate) path: String,
    pub(crate) size: i64,
    pub(crate) modified_ns: i64,
}

/// The sidecars a scan found beside one Version.
#[derive(Debug)]
pub(crate) struct VersionSidecars {
    pub(crate) version: i64,
    /// The Version's file name without its extension, which each sidecar's name starts with.
    pub(crate) stem: String,
    pub(crate) sidecars: Vec<Sidecar>,
}

/// Reads and writes the sidecars stored beside each Version.
#[derive(Debug, Clone)]
pub(crate) struct Sidecars {
    db: Arc<Database>,
}

impl Sidecars {
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// The sidecars stored beside each of `versions` that has any, by Version id, in path order.
    pub(crate) async fn stored(
        &self,
        versions: &[i64],
    ) -> Result<HashMap<i64, Vec<Sidecar>>, sqlx::Error> {
        let versions = serde_json::Value::from(versions).to_string();
        let rows = sqlx::query!(
            "SELECT version_id, path, size, modified_ns FROM sidecar_subtitles
             WHERE version_id IN (SELECT value FROM json_each(?))
             ORDER BY path",
            versions
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut stored: HashMap<i64, Vec<Sidecar>> = HashMap::new();
        for row in rows {
            stored.entry(row.version_id).or_default().push(Sidecar {
                path: row.path,
                size: row.size,
                modified_ns: row.modified_ns,
            });
        }
        Ok(stored)
    }

    /// Appends to the Tracks of each of `probes`, by Version id, the sidecars stored beside it, in
    /// path order, after the file's own, so those keep their places.
    pub(super) async fn append_to(
        &self,
        probes: impl IntoIterator<Item = (i64, &mut Probe)>,
    ) -> Result<(), sqlx::Error> {
        let mut probes: HashMap<i64, &mut Probe> = probes.into_iter().collect();
        let versions: Vec<i64> = probes.keys().copied().collect();
        let versions = serde_json::Value::from(versions).to_string();
        let rows = sqlx::query!(
            r#"SELECT version_id, path, size, modified_ns, codec, language, title,
                      is_default AS "is_default: bool", forced AS "forced: bool", sdh AS "sdh: bool"
               FROM sidecar_subtitles
               WHERE version_id IN (SELECT value FROM json_each(?))
               ORDER BY path"#,
            versions
        )
        .fetch_all(self.db.reader())
        .await?;
        for row in rows {
            let sidecar = Sidecar {
                path: row.path,
                size: row.size,
                modified_ns: row.modified_ns,
            };
            let track = Track {
                kind: subtitle(&row.codec, Some(sidecar)),
                codec: row.codec,
                language: row.language,
                title: row.title,
                default: row.is_default,
                forced: row.forced,
                sdh: row.sdh,
            };
            if let Some(probe) = probes.get_mut(&row.version_id) {
                probe.tracks.push(track);
            }
        }
        Ok(())
    }

    /// Replaces the stored sidecars of each Version in `found` with those found beside it, in
    /// one write.
    pub(crate) async fn replace(&self, found: Vec<VersionSidecars>) -> Result<(), sqlx::Error> {
        self.db.write(move |tx| Box::pin(replace(tx, found))).await
    }
}

async fn replace(
    conn: &mut SqliteConnection,
    found: Vec<VersionSidecars>,
) -> Result<(), sqlx::Error> {
    for beside in found {
        replace_beside(conn, beside).await?;
    }
    Ok(())
}

async fn replace_beside(
    conn: &mut SqliteConnection,
    beside: VersionSidecars,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM sidecar_subtitles WHERE version_id = ?",
        beside.version
    )
    .execute(&mut *conn)
    .await?;
    for sidecar in beside.sidecars {
        let name = Path::new(&sidecar.path).file_name().and_then(OsStr::to_str);
        let Some(track) = name.and_then(|name| track_named(&beside.stem, name)) else {
            continue;
        };
        sqlx::query!(
            "INSERT INTO sidecar_subtitles (version_id, path, size, modified_ns, codec, language,
                                            title, is_default, forced, sdh)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            beside.version,
            sidecar.path,
            sidecar.size,
            sidecar.modified_ns,
            track.codec,
            track.language,
            track.title,
            track.default,
            track.forced,
            track.sdh,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Whether `path` is a sidecar subtitle's, by its extension.
pub(crate) fn is_sidecar(path: &Path) -> bool {
    (path.extension())
        .and_then(OsStr::to_str)
        .and_then(codec_of)
        .is_some()
}

/// The codec, as ffprobe names it, of a sidecar with the extension `extension`, if it is one.
fn codec_of(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "srt" => Some("subrip"),
        "vtt" => Some("webvtt"),
        "ass" => Some("ass"),
        "ssa" => Some("ssa"),
        "sup" => Some("hdmv_pgs_subtitle"),
        _ => None,
    }
}

/// The kind of Track a sidecar of `codec` is: only `.sup`'s is an image.
fn subtitle(codec: &str, sidecar: Option<Sidecar>) -> TrackKind {
    TrackKind::Subtitle {
        image: codec == "hdmv_pgs_subtitle",
        header: false,
        sidecar,
    }
}

impl Track {
    /// The sidecar it's read from, or `None` for one inside the Version's file.
    pub(super) const fn sidecar(&self) -> Option<&Sidecar> {
        match &self.kind {
            TrackKind::Subtitle { sidecar, .. } => sidecar.as_ref(),
            TrackKind::Video { .. } | TrackKind::Audio { .. } => None,
        }
    }
}

/// The Track the sidecar named `name` is, beside a video whose name without its extension is
/// `stem`: its codec from its extension, and its language, flags and title from the `.`-separated
/// tags between the two, as Bazarr writes them. `None` if `name` isn't a sidecar's.
fn track_named(stem: &str, name: &str) -> Option<Track> {
    let (rest, extension) = name.rsplit_once('.')?;
    let codec = codec_of(extension)?;
    let mut track = Track {
        codec: codec.to_owned(),
        language: None,
        title: None,
        default: false,
        forced: false,
        sdh: false,
        kind: subtitle(codec, None),
    };
    let tags = rest.strip_prefix(stem).unwrap_or_default();
    let mut title = Vec::new();
    for tag in tags.split('.').filter(|tag| !tag.is_empty()) {
        match tag.to_ascii_lowercase().as_str() {
            "forced" => track.forced = true,
            "sdh" | "cc" => track.sdh = true,
            // Bazarr's hearing-impaired tag follows the language; alone, `hi` is Hindi.
            "hi" if track.language.is_some() => track.sdh = true,
            "default" => track.default = true,
            _ if track.language.is_none() && names_language(tag) => {
                track.language = Some(tag.to_owned());
            }
            _ => title.push(tag),
        }
    }
    track.title = (!title.is_empty()).then(|| title.join("."));
    Some(track)
}

/// Whether `tag` names a language: an ISO 639 code, or one joined by `-` to a region, such as
/// `pt-BR` or `es-419`. Unlike a Preference's, `und` counts, as it does in a file's own Tracks.
fn names_language(tag: &str) -> bool {
    let is_region = |region: &str| {
        (region.len() == 2 && region.bytes().all(|b| b.is_ascii_alphabetic()))
            || (region.len() == 3 && region.bytes().all(|b| b.is_ascii_digit()))
    };
    let is_language = |code: &str| language(code).is_some();
    is_language(tag)
        || tag
            .split_once('-')
            .is_some_and(|(code, region)| is_language(code) && is_region(region))
}

#[cfg(test)]
impl Sidecars {
    /// The sidecars stored beside the Version `version`, each with the Track its name says, in
    /// path order.
    pub(crate) async fn of(&self, version: i64) -> Vec<(Sidecar, Track)> {
        let rows = sqlx::query!(
            r#"SELECT path, size, modified_ns, codec, language, title,
                      is_default AS "is_default: bool", forced AS "forced: bool", sdh AS "sdh: bool"
               FROM sidecar_subtitles WHERE version_id = ? ORDER BY path"#,
            version
        )
        .fetch_all(self.db.reader())
        .await
        .unwrap();
        (rows.into_iter())
            .map(|row| {
                let track = Track {
                    kind: subtitle(&row.codec, None),
                    codec: row.codec,
                    language: row.language,
                    title: row.title,
                    default: row.is_default,
                    forced: row.forced,
                    sdh: row.sdh,
                };
                let sidecar = Sidecar {
                    path: row.path,
                    size: row.size,
                    modified_ns: row.modified_ns,
                };
                (sidecar, track)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "sidecars_tests.rs"]
mod tests;
