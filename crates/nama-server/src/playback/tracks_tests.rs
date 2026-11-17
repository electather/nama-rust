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
