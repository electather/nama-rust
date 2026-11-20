use nama_proto::nama::v1::{PlaybackReasonKind as Kind, SubtitleFormat};

use crate::ffmpeg::{BurnIn, ImageSubtitle, TextSubtitle};
use crate::playback::{
    DeviceDecoding, Facts, Method, Sidecar, Subtitle, build,
    tests::{
        MKV, MP4, audio, decided, facts, firefox, in_container, kinds, reason_of, remuxing,
        subtitle, text_subtitle, track, transcoded, video,
    },
    tracks::{Track, TrackKind},
};

#[test]
fn a_chosen_audio_track_other_than_the_files_own_is_remuxed_and_the_plan_says_so() {
    let (english, commentary) = (audio("aac", true), audio("aac", false));
    let mut facts = Facts {
        audio: Some(3),
        ..facts(Some(vec![
            video("h264"),
            english,
            subtitle(),
            commentary.clone(),
        ]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(1, None)), "counting only audio");
    assert_eq!(plan.tracks, [video("h264"), commentary]);
    assert_eq!(
        kinds(&plan),
        [Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN]
    );
    assert_eq!(decided(&plan), [true]);
}

#[test]
fn choosing_the_files_own_audio_track_or_no_audio_track_plays_the_files_own() {
    let (mp3, english) = (audio("mp3", false), audio("aac", true));
    let tracks = vec![video("h264"), mp3, english.clone(), subtitle()];
    for chosen in [Some(2), Some(0), Some(3), Some(9)] {
        let mut facts = Facts {
            audio: chosen,
            ..facts(Some(tracks.clone()))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{chosen:?}");
        assert_eq!(plan.tracks, [video("h264"), english.clone()]);
    }
}

#[test]
fn a_chosen_audio_track_the_device_cant_decode_is_converted_to_aac() {
    let mut facts = Facts {
        audio: Some(2),
        ..facts(Some(vec![
            video("h264"),
            audio("aac", true),
            audio("dts", false),
        ]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(1, Some(2))));
    assert_eq!(plan.tracks[1], audio("aac", false));
}

#[test]
fn a_direct_play_or_remux_offers_the_text_subtitles_by_their_place_but_not_image_or_teletext() {
    let english = text_subtitle("subrip");
    let pgs = track(
        "hdmv_pgs_subtitle",
        false,
        TrackKind::Subtitle {
            image: true,
            header: false,
            sidecar: None,
        },
    );
    let teletext = text_subtitle("dvb_teletext");
    let french = Track {
        language: Some("fre".to_owned()),
        ..english.clone()
    };
    let tracks = vec![
        video("h264"),
        english.clone(),
        audio("aac", true),
        pgs,
        teletext,
        french.clone(),
    ];

    let offered = [
        Subtitle {
            track: english,
            place: 1,
            format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
            url: "/media/versions/7/subtitles/1".to_owned(),
            font_urls: vec![],
        },
        Subtitle {
            track: french,
            place: 5,
            format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
            url: "/media/versions/7/subtitles/5".to_owned(),
            font_urls: vec![],
        },
    ];

    for (container, method) in [(MP4, Method::DirectPlay), (MKV, Method::Remux)] {
        let plan = build(&mut in_container(container, Some(tracks.clone())));

        assert_eq!(plan.method(), Some(method), "{container}");
        assert_eq!(plan.subtitles, offered, "{container}");
    }
}

fn pgs() -> Track {
    image_subtitle("hdmv_pgs_subtitle")
}

fn image_subtitle(codec: &str) -> Track {
    track(
        codec,
        false,
        TrackKind::Subtitle {
            image: true,
            header: false,
            sidecar: None,
        },
    )
}

/// A Device that draws PGS subtitles itself.
fn drawing_pgs() -> DeviceDecoding {
    DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_PGS],
        ..firefox()
    }
}

#[test]
fn a_chosen_subtitle_the_device_cant_draw_is_burned_in_by_a_transcode_and_the_plan_names_it() {
    let cases = [
        (pgs(), firefox(), "PGS"),
        (vobsub(), drawing_pgs(), "VobSub"),
        (image_subtitle("dvb_subtitle"), drawing_pgs(), "DVB"),
        (image_subtitle("xsub"), drawing_pgs(), "XSUB"),
        (ass(), drawing_pgs(), "ASS"),
    ];
    for (chosen, device, name) in cases {
        let mut facts = Facts {
            subtitle: Some(2),
            device,
            ..facts(Some(vec![
                video("h264"),
                audio("aac", true),
                chosen.clone(),
            ]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode), "{}", chosen.codec);
        assert!(
            reason_of(&plan, Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN).decided,
            "{}",
            chosen.codec
        );
        assert_eq!(plan.burn_in, Some(2), "{name}");
        assert_eq!(plan.offers.burn_in, vec![2], "{name}");
    }
}

#[test]
fn a_device_that_draws_pgs_shows_each_pgs_track_itself_even_when_the_plan_transcodes() {
    for (video_codec, method) in [("h264", Method::DirectPlay), ("hevc", Method::Transcode)] {
        let tracks = vec![video(video_codec), audio("aac", true), pgs(), subtitle()];
        let mut facts = Facts {
            device: drawing_pgs(),
            ..facts(Some(tracks))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(method), "{video_codec}");
        assert_eq!(
            plan.subtitles,
            [
                Subtitle {
                    track: pgs(),
                    place: 2,
                    format: SubtitleFormat::SUBTITLE_FORMAT_PGS,
                    url: "/media/versions/7/subtitles/2".to_owned(),
                    font_urls: vec![],
                },
                Subtitle {
                    track: subtitle(),
                    place: 3,
                    format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
                    url: "/media/versions/7/subtitles/3".to_owned(),
                    font_urls: vec![],
                },
            ],
            "{video_codec}"
        );
        assert_eq!(plan.offers.burn_in, Vec::<usize>::new(), "{video_codec}");
    }
}

#[test]
fn a_pgs_track_asked_to_burn_in_on_a_device_that_draws_it_burns_nothing_in() {
    let mut facts = Facts {
        subtitle: Some(2),
        device: drawing_pgs(),
        ..facts(Some(vec![video("h264"), audio("aac", true), pgs()]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert!(
        (plan.reasons.iter())
            .all(|reason| reason.kind != Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN)
    );
    assert_eq!(plan.burn_in, None);
}

/// A VobSub Track whose file carries its header.
fn vobsub() -> Track {
    track(
        "dvd_subtitle",
        false,
        TrackKind::Subtitle {
            image: true,
            header: true,
            sidecar: None,
        },
    )
}

/// A Device that draws VobSub subtitles itself.
fn drawing_vobsub() -> DeviceDecoding {
    DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_VOBSUB],
        ..firefox()
    }
}

#[test]
fn a_device_that_draws_vobsub_shows_each_vobsub_track_with_its_header_itself_even_when_transcoding()
{
    for (video_codec, method) in [("h264", Method::DirectPlay), ("hevc", Method::Transcode)] {
        let tracks = vec![video(video_codec), audio("aac", true), vobsub()];
        let mut facts = Facts {
            device: drawing_vobsub(),
            ..facts(Some(tracks))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(method), "{video_codec}");
        assert_eq!(
            plan.subtitles,
            [Subtitle {
                track: vobsub(),
                place: 2,
                format: SubtitleFormat::SUBTITLE_FORMAT_VOBSUB,
                url: "/media/versions/7/subtitles/2".to_owned(),
                font_urls: vec![],
            }],
            "{video_codec}"
        );
        assert_eq!(plan.offers.burn_in, Vec::<usize>::new(), "{video_codec}");
    }
}

#[test]
fn a_vobsub_track_without_its_header_is_burned_in_on_a_device_that_lists_vobsub() {
    let mut facts = Facts {
        subtitle: Some(2),
        device: drawing_vobsub(),
        ..facts(Some(vec![
            video("h264"),
            audio("aac", true),
            image_subtitle("dvd_subtitle"),
        ]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    let reason = reason_of(
        &plan,
        Kind::PLAYBACK_REASON_KIND_SUBTITLES_TRACK_CARRIES_NO_PALETTE,
    );
    assert!(reason.decided);
    assert_eq!(
        reason.text,
        "This Track carries no palette to draw its VobSub subtitles with, so the ones chosen \
         are burned into its picture, which only a Transcode does"
    );
    assert_eq!(plan.subtitles, []);
    assert_eq!(plan.burn_in, Some(2));
    assert_eq!(plan.offers.burn_in, vec![2]);
}

fn ass() -> Track {
    text_subtitle("ass")
}

/// A Device that draws ASS subtitles itself.
fn drawing_ass() -> DeviceDecoding {
    DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_ASS],
        ..firefox()
    }
}

/// [`facts`] for an H.264 and AAC MP4 with an ASS Track third, whose file carries the fonts
/// `font_attachments`, played on `device` with the Track at `subtitle` asked to burn in.
fn with_ass(
    font_attachments: Vec<usize>,
    device: DeviceDecoding,
    subtitle: Option<usize>,
) -> Facts {
    let mut facts = Facts {
        device,
        subtitle,
        ..facts(Some(vec![video("h264"), audio("aac", true), ass()]))
    };
    if let Some(probe) = &mut facts.probe {
        probe.font_attachments = font_attachments;
    }
    facts
}

#[test]
fn an_ass_track_in_a_file_with_fonts_is_drawn_with_them_on_a_device_that_lists_ass() {
    let mut facts = with_ass(vec![1, 3], drawing_ass(), Some(2));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert_eq!(
        plan.subtitles,
        [Subtitle {
            track: ass(),
            place: 2,
            format: SubtitleFormat::SUBTITLE_FORMAT_ASS,
            url: "/media/versions/7/subtitles/2/ass".to_owned(),
            font_urls: vec![
                "/media/versions/7/fonts/0".to_owned(),
                "/media/versions/7/fonts/1".to_owned(),
            ],
        }]
    );
    assert_eq!(plan.burn_in, None);
    assert_eq!(plan.offers.burn_in, Vec::<usize>::new());
}

#[test]
fn an_ass_track_in_a_file_without_fonts_is_burned_in_on_a_device_that_lists_ass() {
    let mut facts = with_ass(vec![], drawing_ass(), Some(2));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert!(
        reason_of(
            &plan,
            Kind::PLAYBACK_REASON_KIND_SUBTITLES_FILE_CARRIES_NO_FONTS
        )
        .decided
    );
    assert_eq!(plan.burn_in, Some(2));
    assert_eq!(plan.offers.burn_in, vec![2]);
    assert_eq!(
        plan.subtitles[0].format,
        SubtitleFormat::SUBTITLE_FORMAT_WEBVTT
    );
    assert_eq!(plan.subtitles[0].url, "/media/versions/7/subtitles/2");
}

#[test]
fn an_ass_track_in_a_file_with_fonts_keeps_both_entries_on_a_device_that_doesnt_list_ass() {
    let plan = build(&mut with_ass(vec![0], drawing_pgs(), None));

    assert_eq!(
        plan.subtitles,
        [Subtitle {
            track: ass(),
            place: 2,
            format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
            url: "/media/versions/7/subtitles/2".to_owned(),
            font_urls: vec![],
        }]
    );
    assert_eq!(plan.offers.burn_in, vec![2]);
}

#[test]
fn an_ass_sidecar_beside_a_file_with_fonts_is_burned_in_from_its_path_on_a_device_that_lists_ass() {
    let sidecar = Track {
        kind: TrackKind::Subtitle {
            image: false,
            header: false,
            sidecar: Some(Sidecar {
                path: "/media/Alien.en.ass".to_owned(),
                size: 1,
                modified_ns: 2,
            }),
        },
        ..ass()
    };
    let mut facts = with_ass(vec![1], drawing_ass(), Some(3));
    if let Some(probe) = &mut facts.probe {
        probe.tracks.push(sidecar);
    }

    let plan = build(&mut facts);

    assert!(
        reason_of(
            &plan,
            Kind::PLAYBACK_REASON_KIND_SUBTITLES_FILE_CARRIES_NO_FONTS
        )
        .decided
    );
    assert_eq!(plan.burn_in, Some(3));
    let shown: Vec<_> = (plan.subtitles.iter())
        .map(|subtitle| (subtitle.place, subtitle.format, subtitle.font_urls.len()))
        .collect();
    assert_eq!(
        shown,
        [
            (2, SubtitleFormat::SUBTITLE_FORMAT_ASS, 1),
            (3, SubtitleFormat::SUBTITLE_FORMAT_WEBVTT, 0)
        ]
    );
    assert_eq!(
        transcoded(&plan).video.burn_in,
        Some(BurnIn::Text {
            subtitle: TextSubtitle::Sidecar {
                path: "/media/Alien.en.ass".to_owned(),
                charset: None,
            }
        })
    );
}

#[test]
fn a_pgs_sidecar_is_burned_in_from_its_path_on_a_device_that_doesnt_draw_pgs() {
    let sidecar = Track {
        kind: TrackKind::Subtitle {
            image: true,
            header: false,
            sidecar: Some(Sidecar {
                path: "/media/Alien.en.sup".to_owned(),
                size: 1,
                modified_ns: 2,
            }),
        },
        ..pgs()
    };
    let tracks = vec![video("h264"), audio("aac", true), pgs(), pgs(), sidecar];
    let mut facts = Facts {
        subtitle: Some(4),
        ..facts(Some(tracks))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.burn_in, Some(4));
    let burn_in = transcoded(&plan).video.burn_in;
    assert!(
        matches!(
            &burn_in,
            Some(BurnIn::Image { subtitle: ImageSubtitle::Sidecar(path), .. }) if path == "/media/Alien.en.sup"
        ),
        "{burn_in:?}"
    );
}

#[test]
fn a_subtitle_choice_that_isnt_an_image_or_ass_subtitle_burns_nothing_in() {
    let teletext = text_subtitle("dvb_teletext");
    let tracks = vec![video("h264"), audio("aac", true), teletext, subtitle()];
    for chosen in [None, Some(1), Some(2), Some(3), Some(9)] {
        let mut facts = Facts {
            subtitle: chosen,
            ..facts(Some(tracks.clone()))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{chosen:?}");
        assert_eq!(plan.reasons, [], "{chosen:?}");
    }
}
