use super::*;

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
    pub(in crate::playback) async fn failure_reason_of(&self, version: i64) -> Option<String> {
        sqlx::query_scalar!(
            "SELECT reason FROM probe_failures WHERE version_id = ?",
            version
        )
        .fetch_optional(self.db.reader())
        .await
        .unwrap()
    }

    /// Marks the Version `version`'s failures, to probe it or list its keyframes, as at the
    /// failure version before the current one.
    pub(in crate::playback) async fn failed_by_older_failure_version(&self, version: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE probe_failures SET failure_version = ? - 1 WHERE version_id = ?",
                        PROBE_FAILURE_VERSION,
                        version
                    )
                    .execute(&mut **tx)
                    .await?;
                    sqlx::query!(
                        "UPDATE version_probes SET keyframes_failure_version = ? - 1
                         WHERE version_id = ? AND keyframes_failure_version IS NOT NULL",
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

    /// Forgets when the Version `version`'s file starts, as its keyframes kept before that was
    /// kept beside them left it.
    pub(in crate::playback) async fn forget_keyframes_start(&self, version: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE version_probes SET keyframes_start_ms = NULL WHERE version_id = ?",
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
    pub(in crate::playback) async fn failed_by_older_probe(&self, version: i64) {
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
    pub(in crate::playback) async fn read_by_older_probe(&self, version: i64) {
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
    let subtitle = |image| {
        track(TrackKind::Subtitle {
            image,
            header: false,
            sidecar: None,
        })
    };
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

#[test]
fn ffmpeg_index_of_an_embedded_subtitle_ignores_sidecars_after_it_and_a_sidecar_has_none() {
    let subtitle = |sidecar| {
        track(TrackKind::Subtitle {
            image: false,
            header: false,
            sidecar,
        })
    };
    let sidecar = || {
        subtitle(Some(Sidecar {
            path: "Movie.en.srt".to_owned(),
            size: 1,
            modified_ns: 2,
        }))
    };
    let probe = Probe {
        container: String::new(),
        duration_ms: None,
        bitrate: None,
        font_attachments: vec![],
        tracks: vec![subtitle(None), subtitle(None), sidecar(), sidecar()],
    };

    let streams: Vec<_> = (0..probe.tracks.len())
        .map(|place| probe.ffmpeg_index(place))
        .collect();

    assert_eq!(streams, [Some(0), Some(1), None, None]);
}
