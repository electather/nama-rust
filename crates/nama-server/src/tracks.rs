//! Tracks: what ffprobe found inside each Version, the facts of its whole file, its video, audio
//! and subtitle Tracks and the fonts it carries.
//!
//! [`Tracks`] owns `version_probes`, `tracks` and `probe_failures` (see the crate's `AGENTS.md`).
//! Their rows are keyed by Version id and go when the file goes; which files there are is for
//! [`crate::contents::Contents`] to say.

use std::{
    collections::{HashMap, HashSet},
    mem::discriminant,
    sync::Arc,
};

use nama_proto::nama::v1 as proto;
use sqlx::SqliteConnection;

use crate::{contents::Stored, db::Database, ffmpeg::TEXT_SUBTITLES};

/// The version of how probing reads a file, stored with what it read like the name parser's
/// (ADR 0023). A Version read by an older one is probed again; when to bump it is in `AGENTS.md`.
pub(crate) const PROBE_VERSION: i64 = 7;

/// Like [`PROBE_VERSION`], but a Version ffprobe lastingly failed on at an older one is tried
/// again, and those it read are left alone; when to bump it is in `AGENTS.md`. A
/// [`PROBE_VERSION`] bump retries them too.
pub(crate) const PROBE_FAILURE_VERSION: i64 = 1;

/// How many times in a row the probe pass tries a file that fails for a passing reason, such as a
/// timeout, before it skips it like a lasting failure. A file that timed out once may well again,
/// and each try can hold a probe slot for the whole timeout.
pub(crate) const PROBE_ATTEMPTS: i64 = 3;

/// What a failure to probe a Version means for trying it again. Only `crate::probing` decides
/// which a failure is; Tracks stores the last two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeFailure {
    /// It says nothing about the file and would fail the same on every other, as when ffprobe
    /// can't be run, so the pass stops and the next one tries them all.
    StopsPass,
    /// It says nothing about the file, as when the file couldn't be opened, so it's tried again
    /// next pass.
    Retried,
    /// The file will fail the same way until it changes.
    Lasting(LastingReason),
    /// It may not happen next time, so it's tried again up to [`PROBE_ATTEMPTS`] times.
    Passing(PassingReason),
}

/// Why ffprobe lastingly couldn't read a file, as `probe_failures.reason` stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LastingReason {
    /// ffprobe read the file and refused it.
    Failed,
    /// ffprobe said something probing can't parse.
    UnreadableOutput,
}

impl LastingReason {
    /// Every one, which [`Tracks::current`] skips.
    const ALL: [Self; 2] = match Self::Failed {
        // Names every variant, so a new one fails to compile here until it's listed.
        Self::Failed | Self::UnreadableOutput => [Self::Failed, Self::UnreadableOutput],
    };

    const fn stored(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::UnreadableOutput => "unreadable_output",
        }
    }
}

/// Why ffprobe couldn't read a file this time, as `probe_failures.reason` stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassingReason {
    /// ffprobe ran past its timeout, as on a slow or unmounted share or a busy disk.
    TimedOut,
}

impl PassingReason {
    const fn stored(self) -> &'static str {
        match self {
            Self::TimedOut => "timed_out",
        }
    }
}

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
    /// The fonts the file carries, each by its place among the file's attachments, which is how
    /// ffmpeg picks one out.
    pub(crate) font_attachments: Vec<usize>,
}

impl Probe {
    /// The place of the Track at `place` among the file's Tracks of its kind, counting from 0,
    /// which is how ffmpeg picks out an audio or subtitle stream. `None` if there's no Track there.
    /// A video Track's can fall short of ffmpeg's, since probing leaves out cover art.
    pub(crate) fn ffmpeg_index(&self, place: usize) -> Option<usize> {
        let kind = discriminant(&self.tracks.get(place)?.kind);
        // Probing keeps every audio and subtitle stream, so ffmpeg's are these, in order.
        Some(
            (self.tracks.iter().take(place))
                .filter(|earlier| discriminant(&earlier.kind) == kind)
                .count(),
        )
    }
}

/// One video, audio or subtitle stream inside a Version.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a separate flag the file sets on the Track, as ffprobe's disposition and \
              the proto's `Track` have them"
)]
pub(crate) struct Track {
    pub(crate) codec: String,
    /// As the file tags it, usually ISO 639-2, such as `eng`.
    pub(crate) language: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) default: bool,
    pub(crate) forced: bool,
    /// Marked for the deaf and hard of hearing, as subtitles that also describe sounds.
    pub(crate) sdh: bool,
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

impl Track {
    /// Whether it's a text subtitle, such as SRT, which a browser shows as WebVTT, unlike an image
    /// one such as PGS.
    pub(crate) fn is_text_subtitle(&self) -> bool {
        matches!(self.kind, TrackKind::Subtitle { .. })
            && TEXT_SUBTITLES.contains(&self.codec.as_str())
    }

    /// Whether it's an ASS or SSA subtitle, whose styling only libass draws.
    pub(crate) fn is_ass_subtitle(&self) -> bool {
        matches!(self.kind, TrackKind::Subtitle { .. })
            && matches!(self.codec.as_str(), "ass" | "ssa")
    }

    /// Whether it's a PGS subtitle, Blu-ray's image ones.
    pub(crate) fn is_pgs_subtitle(&self) -> bool {
        matches!(self.kind, TrackKind::Subtitle { .. }) && self.codec == "hdmv_pgs_subtitle"
    }
}

/// How a video [`Track`]'s picture is HDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hdr {
    Hdr10,
    Hlg,
    DolbyVision {
        /// Such as 5, 7 or 8, which says which players can show it.
        profile: i64,
        /// What a player without Dolby Vision shows instead, and a Transcode makes, or `None` when
        /// only a Dolby Vision player can show it.
        base_layer: Option<BaseLayer>,
    },
}

/// A Dolby Vision Track's Base Layer: the picture a player without Dolby Vision shows instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseLayer {
    Hdr10,
    Hlg,
    Sdr,
}

impl BaseLayer {
    /// How it's HDR, or `None` for SDR.
    pub(crate) const fn hdr(self) -> Option<Hdr> {
        match self {
            Self::Hdr10 => Some(Hdr::Hdr10),
            Self::Hlg => Some(Hdr::Hlg),
            Self::Sdr => None,
        }
    }
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

    /// Those of `files` the current [`PROBE_VERSION`] read, or failed to read for a
    /// [`LastingReason`] or [`PROBE_ATTEMPTS`] times at the current [`PROBE_FAILURE_VERSION`], at
    /// the size and modified time `Contents` stores for the file now, which the probe pass skips.
    /// A failed one has no Tracks, so [`Self::of`] leaves it out.
    pub(crate) async fn current(&self, files: &[Stored]) -> Result<HashSet<i64>, sqlx::Error> {
        let versions: Vec<i64> = files.iter().map(|file| file.id).collect();
        let versions = json_array(&versions);
        let lasting = json_array(&LastingReason::ALL.map(LastingReason::stored));
        let read_at: HashSet<(i64, i64, i64)> = sqlx::query!(
            r#"SELECT version_id AS "version_id!", size AS "size!", modified_ns AS "modified_ns!"
               FROM version_probes
               WHERE probe_version = ?1 AND version_id IN (SELECT value FROM json_each(?2))
               UNION ALL
               SELECT version_id, size, modified_ns FROM probe_failures
               WHERE probe_version = ?1 AND failure_version = ?3
                 AND (reason IN (SELECT value FROM json_each(?5)) OR attempts >= ?4)
                 AND version_id IN (SELECT value FROM json_each(?2))"#,
            PROBE_VERSION,
            versions,
            PROBE_FAILURE_VERSION,
            PROBE_ATTEMPTS,
            lasting
        )
        .fetch_all(self.db.reader())
        .await?
        .into_iter()
        .map(|row| (row.version_id, row.size, row.modified_ns))
        .collect();
        Ok(files
            .iter()
            .filter(|file| read_at.contains(&(file.id, file.size, file.modified_ns)))
            .map(|file| file.id)
            .collect())
    }

    /// Stores `probe` as what the current [`PROBE_VERSION`] found in the Version `version` at its
    /// stored size and modified time, replacing what was stored for it. A Version removed since it
    /// was probed stores nothing.
    pub(crate) async fn store(&self, version: Stored, probe: Probe) -> Result<(), sqlx::Error> {
        let stored = self
            .db
            .write(move |tx| Box::pin(store(tx, version, probe)))
            .await;
        ignore_removed(stored)
    }

    /// Stores that the current [`PROBE_VERSION`] couldn't read the Version `version` at its stored
    /// size and modified time, and why (`failure`). For a lasting reason, the probe pass skips it
    /// until one of them or [`PROBE_FAILURE_VERSION`] changes; for a passing one, it tries again
    /// next pass, until it has failed [`PROBE_ATTEMPTS`] times in a row. Forgets what probing found
    /// in it before, so Tracks read from what the file held then don't outlive it. A Version
    /// removed since, or a `failure` that says nothing about the file, stores and forgets nothing.
    pub(crate) async fn store_failure(
        &self,
        version: Stored,
        failure: ProbeFailure,
    ) -> Result<(), sqlx::Error> {
        let reason = match failure {
            ProbeFailure::StopsPass | ProbeFailure::Retried => return Ok(()),
            ProbeFailure::Lasting(reason) => reason.stored(),
            ProbeFailure::Passing(reason) => reason.stored(),
        };
        let stored = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "DELETE FROM version_probes WHERE version_id = ?",
                        version.id
                    )
                    .execute(&mut **tx)
                    .await?;
                    sqlx::query!(
                        // SET reads the row as it was, so `attempts` counts on only when the
                        // file and versions it failed at before are the same.
                        "INSERT INTO probe_failures
                             (version_id, reason, probe_version, failure_version, size,
                              modified_ns, attempts)
                         VALUES (?, ?, ?, ?, ?, ?, 1)
                         ON CONFLICT (version_id) DO UPDATE SET
                             attempts = CASE
                                 WHEN (probe_version, failure_version, size, modified_ns)
                                    = (excluded.probe_version, excluded.failure_version,
                                       excluded.size, excluded.modified_ns)
                                 THEN attempts + 1 ELSE 1 END,
                             reason = excluded.reason,
                             probe_version = excluded.probe_version,
                             failure_version = excluded.failure_version,
                             size = excluded.size,
                             modified_ns = excluded.modified_ns",
                        version.id,
                        reason,
                        PROBE_VERSION,
                        PROBE_FAILURE_VERSION,
                        version.size,
                        version.modified_ns,
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await;
        ignore_removed(stored)
    }

    /// When each keyframe of the stored file `file`'s first video falls, in milliseconds, if
    /// they've been kept since it was last probed at `file`'s size and modified time.
    pub(crate) async fn keyframes(&self, file: Stored) -> Result<Option<Vec<i64>>, sqlx::Error> {
        let keyframes = sqlx::query_scalar!(
            "SELECT keyframes_ms FROM version_probes
             WHERE version_id = ? AND size = ? AND modified_ns = ?",
            file.id,
            file.size,
            file.modified_ns
        )
        .fetch_optional(self.db.reader())
        .await?
        .flatten();
        // Only `store_keyframes` writes them, as JSON it made.
        Ok(keyframes.and_then(|json| serde_json::from_str(&json).ok()))
    }

    /// Keeps `keyframes` as the Version `file`'s until it's next probed. A Version not last probed
    /// at `file`'s size and modified time keeps nothing, as they may be the old file's.
    pub(crate) async fn store_keyframes(
        &self,
        file: Stored,
        keyframes: &[i64],
    ) -> Result<(), sqlx::Error> {
        let keyframes = json_array(keyframes);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE version_probes SET keyframes_ms = ?
                         WHERE version_id = ? AND size = ? AND modified_ns = ?",
                        keyframes,
                        file.id,
                        file.size,
                        file.modified_ns
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await
    }

    /// What probing found in each of `files`, by Version id, if probing read the file at the size
    /// and modified time `Contents` stores for it now. An older [`PROBE_VERSION`]'s read still
    /// counts: it read the same bytes, and hiding it would blank every Version's Tracks after a
    /// bump until the probe pass catches up.
    pub(crate) async fn of(&self, files: &[Stored]) -> Result<HashMap<i64, Probe>, sqlx::Error> {
        let read_at: HashSet<(i64, i64, i64)> = files
            .iter()
            .map(|file| (file.id, file.size, file.modified_ns))
            .collect();
        let ids: Vec<i64> = files.iter().map(|file| file.id).collect();
        let versions = json_array(&ids);
        let probes = sqlx::query!(
            "SELECT version_id, size, modified_ns, container, duration_ms, bitrate,
                    font_attachments
             FROM version_probes
             WHERE version_id IN (SELECT value FROM json_each(?))",
            versions
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut probes: HashMap<i64, Probe> = probes
            .into_iter()
            .filter(|row| read_at.contains(&(row.version_id, row.size, row.modified_ns)))
            .map(|row| {
                let probe = Probe {
                    container: row.container,
                    duration_ms: row.duration_ms,
                    bitrate: row.bitrate,
                    tracks: Vec::new(),
                    // Only `store` writes them, as JSON it made.
                    font_attachments: serde_json::from_str(&row.font_attachments)
                        .unwrap_or_default(),
                };
                (row.version_id, probe)
            })
            .collect();
        let tracks = sqlx::query!(
            r#"SELECT version_id, kind, codec, language, title, is_default AS "default: bool",
                      forced AS "forced: bool", sdh AS "sdh: bool", width, height, frame_rate, bit_depth, hdr,
                      dolby_vision_profile,
                      dolby_vision_base_layer,
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
                        row.dolby_vision_base_layer.as_deref(),
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
                sdh: row.sdh,
                kind,
            };
            if let Some(probe) = probes.get_mut(&row.version_id) {
                probe.tracks.push(track);
            }
        }
        Ok(probes)
    }
}

/// `written`, but a write about a Version removed meanwhile is no error: what it said goes with
/// the Version.
fn ignore_removed(written: Result<(), sqlx::Error>) -> Result<(), sqlx::Error> {
    match written {
        Err(sqlx::Error::Database(err)) if err.is_foreign_key_violation() => Ok(()),
        written => written,
    }
}

/// `values` as a JSON array, which SQLite's `json_each` turns into rows, since a query can't bind
/// a list.
fn json_array<T: Clone + Into<serde_json::Value>>(values: &[T]) -> String {
    serde_json::Value::from(values).to_string()
}

async fn store(
    conn: &mut SqliteConnection,
    stored: Stored,
    probe: Probe,
) -> Result<(), sqlx::Error> {
    let version = stored.id;
    let font_attachments = json_array(&probe.font_attachments);
    sqlx::query!("DELETE FROM version_probes WHERE version_id = ?", version)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("DELETE FROM probe_failures WHERE version_id = ?", version)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "INSERT INTO version_probes (version_id, probe_version, size, modified_ns, container,
                                     duration_ms, bitrate, font_attachments)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        version,
        PROBE_VERSION,
        stored.size,
        stored.modified_ns,
        probe.container,
        probe.duration_ms,
        probe.bitrate,
        font_attachments,
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
        let (hdr, dolby_vision_profile, dolby_vision_base_layer) = hdr_columns(hdr);
        sqlx::query!(
            "INSERT INTO tracks (version_id, position, kind, codec, language, title, is_default,
                                 forced, sdh, width, height, frame_rate, bit_depth, hdr,
                                 dolby_vision_profile, dolby_vision_base_layer, channels,
                                 image)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            version,
            position,
            kind,
            track.codec,
            track.language,
            track.title,
            track.default,
            track.forced,
            track.sdh,
            width,
            height,
            frame_rate,
            bit_depth,
            hdr,
            dolby_vision_profile,
            dolby_vision_base_layer,
            channels,
            image,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// `hdr` as its `tracks` columns: its name, and Dolby Vision's profile and Base Layer.
const fn hdr_columns(
    hdr: Option<Hdr>,
) -> (Option<&'static str>, Option<i64>, Option<&'static str>) {
    match hdr {
        None => (None, None, None),
        Some(Hdr::Hdr10) => (Some("hdr10"), None, None),
        Some(Hdr::Hlg) => (Some("hlg"), None, None),
        Some(Hdr::DolbyVision {
            profile,
            base_layer,
        }) => {
            let base_layer = match base_layer {
                None => None,
                Some(BaseLayer::Hdr10) => Some("hdr10"),
                Some(BaseLayer::Hlg) => Some("hlg"),
                Some(BaseLayer::Sdr) => Some("sdr"),
            };
            (Some("dolby_vision"), Some(profile), base_layer)
        }
    }
}

/// The [`Hdr`] that [`hdr_columns`] stored.
fn hdr_from_columns(
    hdr: Option<&str>,
    dolby_vision_profile: Option<i64>,
    dolby_vision_base_layer: Option<&str>,
) -> Option<Hdr> {
    Some(match hdr? {
        "hdr10" => Hdr::Hdr10,
        "hlg" => Hdr::Hlg,
        // The table holds Dolby Vision's facts for it, and only for it.
        _ => Hdr::DolbyVision {
            profile: dolby_vision_profile.unwrap_or_default(),
            base_layer: dolby_vision_base_layer.and_then(|base_layer| match base_layer {
                "hdr10" => Some(BaseLayer::Hdr10),
                "hlg" => Some(BaseLayer::Hlg),
                "sdr" => Some(BaseLayer::Sdr),
                _ => None,
            }),
        },
    })
}

/// `track` as the API shows it.
pub(crate) fn track_to_proto(track: &Track) -> proto::Track {
    // 0 is what the proto says for a fact the file doesn't give. Stored ones are never negative.
    let known = |n: Option<i64>| n.and_then(|n| u32::try_from(n).ok()).unwrap_or_default();
    let kind = match track.kind {
        TrackKind::Video {
            width,
            height,
            frame_rate,
            bit_depth,
            hdr,
        } => {
            let (hdr, dolby_vision) = hdr_to_proto(hdr);
            proto::VideoTrack {
                width: known(width),
                height: known(height),
                frame_rate: frame_rate.unwrap_or_default(),
                bit_depth: known(bit_depth),
                hdr: hdr.into(),
                dolby_vision: dolby_vision.into(),
                ..Default::default()
            }
            .into()
        }
        TrackKind::Audio { channels } => proto::AudioTrack {
            channels: known(channels),
            ..Default::default()
        }
        .into(),
        TrackKind::Subtitle { image } => proto::SubtitleTrack {
            image,
            ..Default::default()
        }
        .into(),
    };
    proto::Track {
        codec: track.codec.clone(),
        language: track.language.clone(),
        title: track.title.clone(),
        default: track.default,
        forced: track.forced,
        sdh: track.sdh,
        kind,
        ..Default::default()
    }
}

/// `hdr` as a video Track's `hdr` and `dolby_vision`.
fn hdr_to_proto(hdr: Option<Hdr>) -> (proto::HdrFormat, Option<proto::DolbyVision>) {
    match hdr {
        None => (proto::HdrFormat::HDR_FORMAT_UNSPECIFIED, None),
        Some(Hdr::Hdr10) => (proto::HdrFormat::HDR_FORMAT_HDR10, None),
        Some(Hdr::Hlg) => (proto::HdrFormat::HDR_FORMAT_HLG, None),
        Some(Hdr::DolbyVision {
            profile,
            base_layer,
        }) => {
            let base_layer = match base_layer {
                None => proto::BaseLayer::BASE_LAYER_UNSPECIFIED,
                Some(BaseLayer::Hdr10) => proto::BaseLayer::BASE_LAYER_HDR10,
                Some(BaseLayer::Hlg) => proto::BaseLayer::BASE_LAYER_HLG,
                Some(BaseLayer::Sdr) => proto::BaseLayer::BASE_LAYER_SDR,
            };
            let dolby_vision = proto::DolbyVision {
                // ffprobe reads it from a byte, so it's never negative.
                profile: u32::try_from(profile).unwrap_or_default(),
                base_layer: base_layer.into(),
                ..Default::default()
            };
            (
                proto::HdrFormat::HDR_FORMAT_DOLBY_VISION,
                Some(dolby_vision),
            )
        }
    }
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

    /// Why probing the Version `version` last failed, as stored, if it did.
    pub(crate) async fn failure_reason_of(&self, version: i64) -> Option<String> {
        sqlx::query_scalar!(
            "SELECT reason FROM probe_failures WHERE version_id = ?",
            version
        )
        .fetch_optional(self.db.reader())
        .await
        .unwrap()
    }

    /// Marks the Version `version` as failed at the failure version before the current one.
    pub(crate) async fn failed_by_older_failure_version(&self, version: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE probe_failures SET failure_version = ? - 1 WHERE version_id = ?",
                        PROBE_FAILURE_VERSION,
                        version
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap();
    }

    /// Marks the Version `version` as failed by the version before the current one.
    pub(crate) async fn failed_by_older_probe(&self, version: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE probe_failures SET probe_version = ? - 1 WHERE version_id = ?",
                        PROBE_VERSION,
                        version
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap();
    }

    /// Marks the Version `version` as probed by the version before the current one.
    pub(crate) async fn read_by_older_probe(&self, version: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE version_probes SET probe_version = ? - 1 WHERE version_id = ?",
                        PROBE_VERSION,
                        version
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(kind: TrackKind) -> Track {
        Track {
            codec: String::new(),
            language: None,
            title: None,
            default: false,
            forced: false,
            sdh: false,
            kind,
        }
    }

    #[test]
    fn ffmpeg_index_counts_only_earlier_tracks_of_the_same_kind() {
        let video = || {
            track(TrackKind::Video {
                width: None,
                height: None,
                frame_rate: None,
                bit_depth: None,
                hdr: None,
            })
        };
        let audio = || track(TrackKind::Audio { channels: None });
        let subtitle = |image| track(TrackKind::Subtitle { image });
        let probe = Probe {
            container: String::new(),
            duration_ms: None,
            bitrate: None,
            font_attachments: vec![],
            tracks: vec![
                video(),
                audio(),
                subtitle(false),
                audio(),
                video(),
                subtitle(true),
                audio(),
            ],
        };

        let streams: Vec<_> = (0..=probe.tracks.len())
            .map(|place| probe.ffmpeg_index(place))
            .collect();

        assert_eq!(
            streams,
            [
                Some(0),
                Some(0),
                Some(0),
                Some(1),
                Some(1),
                Some(1),
                Some(2),
                None
            ]
        );
    }
}
