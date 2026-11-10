//! Tracks: what ffprobe found inside each Version, the facts of its whole file and its video, audio
//! and subtitle Tracks.
//!
//! [`Tracks`] owns `version_probes` and `tracks` (see the crate's `AGENTS.md`). Their rows are
//! keyed by Version id and go when the file goes; which files there are is for
//! [`crate::contents::Contents`] to say.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use sqlx::SqliteConnection;

use crate::db::Database;

/// The version of how probing reads a file, stored with what it read like the name parser's
/// (ADR 0023), so what an older version read can be told apart. Version 1 read no bit depth or
/// HDR format.
pub(crate) const PROBE_VERSION: i64 = 2;

/// What probing a Version found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Probe {
    /// As ffprobe names it, such as `matroska,webm`.
    pub(crate) container: String,
    pub(crate) duration_ms: Option<i64>,
    /// Overall, in bits per second.
    pub(crate) bitrate: Option<i64>,
    /// In the order the file holds them.
    pub(crate) tracks: Vec<Track>,
}

/// One video, audio or subtitle stream inside a Version.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Track {
    pub(crate) codec: String,
    /// As the file tags it, usually ISO 639-2, such as `eng`.
    pub(crate) language: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) default: bool,
    pub(crate) forced: bool,
    pub(crate) kind: TrackKind,
}

/// What kind of stream a [`Track`] is, with the facts only that kind has.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TrackKind {
    Video {
        width: Option<i64>,
        height: Option<i64>,
        /// In frames per second.
        frame_rate: Option<f64>,
        /// Bits per sample, such as 10 for most HDR.
        bit_depth: Option<i64>,
        /// `None` for SDR, or when the file doesn't say.
        hdr: Option<Hdr>,
    },
    Audio {
        channels: Option<i64>,
    },
    Subtitle {
        /// A bitmap, such as PGS, rather than text, such as SRT.
        image: bool,
    },
}

/// How a video [`Track`]'s picture is HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hdr {
    Hdr10,
    Hlg,
    DolbyVision {
        /// Such as 5, 7 or 8, which says which players can show it.
        profile: i64,
        /// Whether its base layer is HDR10, which a player without Dolby Vision shows instead.
        hdr10_base_layer: bool,
    },
}

/// Reads and writes what probing found.
#[derive(Debug, Clone)]
pub(crate) struct Tracks {
    db: Arc<Database>,
}

impl Tracks {
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Those of the Versions `versions` that have been probed.
    pub(crate) async fn probed(&self, versions: &[i64]) -> Result<HashSet<i64>, sqlx::Error> {
        let versions = json_array(versions);
        sqlx::query_scalar!(
            "SELECT version_id FROM version_probes
             WHERE version_id IN (SELECT value FROM json_each(?))",
            versions
        )
        .fetch_all(self.db.reader())
        .await
        .map(|ids| ids.into_iter().collect())
    }

    /// Stores `probe` as what the current [`PROBE_VERSION`] found in the Version `version`,
    /// replacing what was stored for it. A Version removed since it was probed stores nothing.
    pub(crate) async fn store(&self, version: i64, probe: Probe) -> Result<(), sqlx::Error> {
        let stored = self
            .db
            .write(move |tx| Box::pin(store(tx, version, probe)))
            .await;
        match stored {
            Err(sqlx::Error::Database(err)) if err.is_foreign_key_violation() => Ok(()),
            stored => stored,
        }
    }

    /// What probing found in each of the Versions `versions` that has been probed.
    pub(crate) async fn of(&self, versions: &[i64]) -> Result<HashMap<i64, Probe>, sqlx::Error> {
        let versions = json_array(versions);
        let probes = sqlx::query!(
            "SELECT version_id, container, duration_ms, bitrate FROM version_probes
             WHERE version_id IN (SELECT value FROM json_each(?))",
            versions
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut probes: HashMap<i64, Probe> = probes
            .into_iter()
            .map(|row| {
                let probe = Probe {
                    container: row.container,
                    duration_ms: row.duration_ms,
                    bitrate: row.bitrate,
                    tracks: Vec::new(),
                };
                (row.version_id, probe)
            })
            .collect();
        let tracks = sqlx::query!(
            r#"SELECT version_id, kind, codec, language, title, is_default AS "default: bool",
                      forced AS "forced: bool", width, height, frame_rate, bit_depth, hdr,
                      dolby_vision_profile,
                      dolby_vision_hdr10_base_layer AS "dolby_vision_hdr10_base_layer: bool",
                      channels, image AS "image: bool"
               FROM tracks
               WHERE version_id IN (SELECT value FROM json_each(?))
               ORDER BY version_id, position"#,
            versions
        )
        .fetch_all(self.db.reader())
        .await?;
        for row in tracks {
            let kind = match row.kind.as_str() {
                "video" => TrackKind::Video {
                    width: row.width,
                    height: row.height,
                    frame_rate: row.frame_rate,
                    bit_depth: row.bit_depth,
                    hdr: hdr_from_columns(
                        row.hdr.as_deref(),
                        row.dolby_vision_profile,
                        row.dolby_vision_hdr10_base_layer,
                    ),
                },
                "audio" => TrackKind::Audio {
                    channels: row.channels,
                },
                _ => TrackKind::Subtitle {
                    image: row.image.unwrap_or_default(),
                },
            };
            let track = Track {
                codec: row.codec,
                language: row.language,
                title: row.title,
                default: row.default,
                forced: row.forced,
                kind,
            };
            if let Some(probe) = probes.get_mut(&row.version_id) {
                probe.tracks.push(track);
            }
        }
        Ok(probes)
    }
}

/// `ids` as a JSON array, which SQLite's `json_each` turns into rows, since a query can't bind a
/// list.
fn json_array(ids: &[i64]) -> String {
    serde_json::Value::from(ids).to_string()
}

async fn store(conn: &mut SqliteConnection, version: i64, probe: Probe) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM version_probes WHERE version_id = ?", version)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "INSERT INTO version_probes (version_id, probe_version, container, duration_ms, bitrate)
         VALUES (?, ?, ?, ?, ?)",
        version,
        PROBE_VERSION,
        probe.container,
        probe.duration_ms,
        probe.bitrate,
    )
    .execute(&mut *conn)
    .await?;
    for (position, track) in (0_i64..).zip(probe.tracks) {
        let (kind, width, height, frame_rate, bit_depth, hdr, channels, image) = match track.kind {
            TrackKind::Video {
                width,
                height,
                frame_rate,
                bit_depth,
                hdr,
            } => (
                "video", width, height, frame_rate, bit_depth, hdr, None, None,
            ),
            TrackKind::Audio { channels } => {
                ("audio", None, None, None, None, None, channels, None)
            }
            TrackKind::Subtitle { image } => {
                ("subtitle", None, None, None, None, None, None, Some(image))
            }
        };
        let (hdr, dolby_vision_profile, dolby_vision_hdr10_base_layer) = hdr_columns(hdr);
        sqlx::query!(
            "INSERT INTO tracks (version_id, position, kind, codec, language, title, is_default,
                                 forced, width, height, frame_rate, bit_depth, hdr,
                                 dolby_vision_profile, dolby_vision_hdr10_base_layer, channels,
                                 image)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            version,
            position,
            kind,
            track.codec,
            track.language,
            track.title,
            track.default,
            track.forced,
            width,
            height,
            frame_rate,
            bit_depth,
            hdr,
            dolby_vision_profile,
            dolby_vision_hdr10_base_layer,
            channels,
            image,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// `hdr` as its `tracks` columns: its name, and Dolby Vision's profile and whether its base layer
/// is HDR10.
const fn hdr_columns(hdr: Option<Hdr>) -> (Option<&'static str>, Option<i64>, Option<bool>) {
    match hdr {
        None => (None, None, None),
        Some(Hdr::Hdr10) => (Some("hdr10"), None, None),
        Some(Hdr::Hlg) => (Some("hlg"), None, None),
        Some(Hdr::DolbyVision {
            profile,
            hdr10_base_layer,
        }) => (Some("dolby_vision"), Some(profile), Some(hdr10_base_layer)),
    }
}

/// The [`Hdr`] that [`hdr_columns`] stored.
fn hdr_from_columns(
    hdr: Option<&str>,
    dolby_vision_profile: Option<i64>,
    dolby_vision_hdr10_base_layer: Option<bool>,
) -> Option<Hdr> {
    Some(match hdr? {
        "hdr10" => Hdr::Hdr10,
        "hlg" => Hdr::Hlg,
        // The table holds Dolby Vision's facts for it, and only for it.
        _ => Hdr::DolbyVision {
            profile: dolby_vision_profile.unwrap_or_default(),
            hdr10_base_layer: dolby_vision_hdr10_base_layer.unwrap_or_default(),
        },
    })
}

#[cfg(test)]
impl Tracks {
    /// The probe version that read the Version `version`, if it has been probed.
    pub(crate) async fn probe_version_of(&self, version: i64) -> Option<i64> {
        sqlx::query_scalar!(
            "SELECT probe_version FROM version_probes WHERE version_id = ?",
            version
        )
        .fetch_optional(self.db.reader())
        .await
        .unwrap()
    }
}
