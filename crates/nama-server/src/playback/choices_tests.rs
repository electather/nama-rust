use nama_proto::nama::v1::{PlaybackReasonKind as Kind, SubtitleFormat};

use super::{audio_asked, preferred_subtitle};
use crate::playback::{
    AskedAudio, DeviceDecoding, Facts, Method, Subtitle, build,
    tests::{
        MKV, MP4, audio, decided, facts, firefox, in_container, kinds, reason_of, remuxing,
        subtitle, text_subtitle, track, video, without_ffmpeg,
    },
    track_preferences::{Description, Preferred, SubtitlePreference},
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

/// A subtitle Track of `codec` in `language`, forced or not, SDH or not, and titled `title`.
fn subtitle_in(
    codec: &str,
    language: Option<&str>,
    (forced, sdh): (bool, bool),
    title: Option<&str>,
) -> Track {
    let image = codec == "hdmv_pgs_subtitle";
    Track {
        language: language.map(str::to_owned),
        title: title.map(str::to_owned),
        forced,
        sdh,
        ..track(
            codec,
            false,
            TrackKind::Subtitle {
                image,
                header: false,
            },
        )
    }
}

/// The place [`preferred_subtitle`] picks among `tracks`, in a file that carries no fonts.
fn preferred_among(
    preferred: &Preferred<SubtitlePreference>,
    tracks: &[Track],
    device: &DeviceDecoding,
) -> Option<usize> {
    let probe = facts(Some(tracks.to_vec())).probe.unwrap();
    preferred_subtitle(preferred, &probe, device)
}

/// A Show's or Movie's Subtitle Preference for a Track like `track`.
fn like(track: &Track) -> Preferred<SubtitlePreference> {
    Preferred {
        preference: SubtitlePreference::Like(Description::of(track)),
        users_own: false,
        foreign_audio_only: false,
    }
}

/// The User's own Subtitle Preference for a Track like `track`, which keeps no title.
fn users_own_like(track: &Track) -> Preferred<SubtitlePreference> {
    Preferred {
        preference: SubtitlePreference::Like(Description {
            title: None,
            ..Description::of(track)
        }),
        users_own: true,
        foreign_audio_only: false,
    }
}

const NOT_FORCED_OR_SDH: (bool, bool) = (false, false);

#[test]
fn a_preferred_subtitle_must_be_in_its_language_and_as_forced() {
    let device = firefox();
    let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
    let tracks = [
        video("h264"),
        subtitle_in("subrip", Some("eng"), (true, false), None),
        subtitle_in("subrip", Some("ger"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", Some("en"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", None, NOT_FORCED_OR_SDH, None),
    ];

    assert_eq!(preferred_among(&like(&english), &tracks, &device), Some(3));
    let unlabelled = subtitle_in("subrip", None, NOT_FORCED_OR_SDH, None);
    assert_eq!(
        preferred_among(&like(&unlabelled), &tracks, &device),
        Some(4)
    );
    let french = subtitle_in("subrip", Some("fre"), NOT_FORCED_OR_SDH, None);
    assert_eq!(preferred_among(&like(&french), &tracks, &device), None);
    let off = Preferred {
        preference: SubtitlePreference::Off,
        users_own: false,
        foreign_audio_only: false,
    };
    assert_eq!(preferred_among(&off, &tracks, &device), None);
}

#[test]
fn the_users_own_preference_ranks_sdh_then_one_the_device_shows_whatever_the_title() {
    let device = firefox();
    let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
    let english_sdh = subtitle_in("subrip", Some("eng"), (false, true), None);
    let tracks = [
        subtitle_in("subrip", Some("eng"), (false, true), None),
        subtitle_in("hdmv_pgs_subtitle", Some("eng"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("English")),
        subtitle_in("subrip", Some("eng"), (true, false), None),
    ];

    assert_eq!(
        preferred_among(&users_own_like(&english), &tracks, &device),
        Some(2)
    );
    assert_eq!(preferred_among(&like(&english), &tracks, &device), Some(1));
    assert_eq!(
        preferred_among(&users_own_like(&english_sdh), &tracks, &device),
        Some(0)
    );
    assert_eq!(
        preferred_among(&users_own_like(&english), &tracks[3..], &device),
        None,
        "a full Preference never takes a forced-only Track"
    );
}

#[test]
fn a_preferred_subtitle_ranks_sdh_then_title_then_one_the_device_shows_then_the_first() {
    let device = firefox();
    let wanted = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full"));
    let cases = [
        (
            "the same SDH beats the same title",
            vec![
                subtitle_in("subrip", Some("eng"), (false, true), Some("Full")),
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Signs")),
            ],
            Some(1),
        ),
        (
            "the same title, in any case, beats one the Device shows",
            vec![
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None),
                subtitle_in(
                    "hdmv_pgs_subtitle",
                    Some("eng"),
                    NOT_FORCED_OR_SDH,
                    Some("FULL"),
                ),
            ],
            Some(1),
        ),
        (
            "one the Device shows beats one that needs Burn-in",
            vec![
                subtitle_in(
                    "hdmv_pgs_subtitle",
                    Some("eng"),
                    NOT_FORCED_OR_SDH,
                    Some("Full"),
                ),
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
            ],
            Some(1),
        ),
        (
            "a tie goes to the first",
            vec![
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
            ],
            Some(0),
        ),
        (
            "one neither shown nor burned in never matches",
            vec![subtitle_in(
                "dvb_teletext",
                Some("eng"),
                NOT_FORCED_OR_SDH,
                Some("Full"),
            )],
            None,
        ),
    ];

    for (case, tracks, place) in cases {
        let preferred = preferred_among(&like(&wanted), &tracks, &device);
        assert_eq!(preferred, place, "{case}");
    }
}

#[test]
fn a_full_sdh_preference_picks_a_track_titled_sdh_over_a_plain_english_one() {
    let wanted = subtitle_in("subrip", Some("eng"), (false, true), Some("Full"));
    // Probing marks a subtitle titled "English (SDH)" as SDH without its disposition flag.
    let tracks = [
        subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("English")),
        subtitle_in("subrip", Some("eng"), (false, true), Some("English (SDH)")),
    ];

    assert_eq!(
        preferred_among(&like(&wanted), &tracks, &firefox()),
        Some(1)
    );
}

#[test]
fn a_preferred_text_subtitle_is_shown_by_the_device_and_the_plan_starts_with_it() {
    let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
    let tracks = vec![video("h264"), audio("aac", true), english.clone()];
    let mut facts = Facts {
        subtitle_preference: Some(like(&english)),
        ..facts(Some(tracks))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert_eq!((plan.burn_in, plan.starting_subtitle), (None, Some(2)));
}

#[test]
fn a_preferred_image_subtitle_is_burned_in_or_dropped_as_a_chosen_one_is() {
    let pgs = subtitle_in("hdmv_pgs_subtitle", Some("eng"), NOT_FORCED_OR_SDH, None);
    let tracks = vec![video("h264"), audio("aac", true), pgs.clone()];
    let preferred = || Facts {
        subtitle_preference: Some(like(&pgs)),
        ..facts(Some(tracks.clone()))
    };

    let burned_in = build(&mut preferred());
    let mut no_ffmpeg = preferred();
    without_ffmpeg(&mut no_ffmpeg);
    let dropped = build(&mut no_ffmpeg);

    assert_eq!(burned_in.method(), Some(Method::Transcode));
    assert_eq!(
        (burned_in.burn_in, burned_in.starting_subtitle),
        (Some(2), Some(2))
    );
    assert_eq!(dropped.method(), Some(Method::DirectPlay));
    assert_eq!((dropped.burn_in, dropped.starting_subtitle), (None, None));
    assert!(
        reason_of(
            &dropped,
            Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED
        )
        .decided
    );
}

const FORCED: (bool, bool) = (true, false);

/// Full and forced-only English subtitles, at places 3 and 4 after a video and two audio
/// Tracks.
fn english_subtitles() -> [Track; 2] {
    [
        subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", Some("eng"), FORCED, None),
    ]
}

/// The User's own Subtitle Preference for full English subtitles only over audio in another
/// language.
fn english_over_foreign_audio() -> Preferred<SubtitlePreference> {
    let [full, _] = english_subtitles();
    Preferred {
        foreign_audio_only: true,
        ..users_own_like(&full)
    }
}

/// The subtitle a plan starts with by `preferred`, of a Version with a video Track, then the
/// audio Tracks `audio`, then the subtitle Tracks `subtitles`.
fn starting_subtitle(
    preferred: Preferred<SubtitlePreference>,
    audio: &[Track],
    subtitles: &[Track],
) -> Option<usize> {
    let tracks = [&[video("h264")], audio, subtitles].concat();
    let mut facts = Facts {
        subtitle_preference: Some(preferred),
        ..facts(Some(tracks))
    };
    build(&mut facts).starting_subtitle
}

#[test]
fn subtitles_wanted_over_foreign_audio_start_full_over_japanese_or_untagged_audio() {
    let japanese = audio_in("aac", "jpn", None, true);
    let untagged = audio("aac", true);
    let undetermined = audio_in("aac", "und", None, true);
    let not_iso_639 = audio_in("aac", "en-US", None, true);
    let english = audio_in("aac", "eng", None, false);

    for (case, played) in [
        ("Japanese", japanese),
        ("untagged", untagged),
        ("`und`", undetermined),
        ("`en-US`", not_iso_639),
    ] {
        let audio = [played, english.clone()];
        assert_eq!(
            starting_subtitle(english_over_foreign_audio(), &audio, &english_subtitles()),
            Some(3),
            "{case}"
        );
    }
}

#[test]
fn subtitles_wanted_over_foreign_audio_start_forced_or_none_over_audio_in_their_language() {
    let audio = || {
        [
            audio_in("aac", "en", None, true),
            audio_in("aac", "jpn", None, false),
        ]
    };
    let [full, _] = english_subtitles();

    let forced = starting_subtitle(english_over_foreign_audio(), &audio(), &english_subtitles());
    let none = starting_subtitle(english_over_foreign_audio(), &audio(), &[full]);

    assert_eq!(forced, Some(4));
    assert_eq!(none, None);
}

#[test]
fn subtitles_wanted_over_foreign_audio_look_at_the_files_own_once_the_plan_drops_the_asked() {
    let tracks = [
        vec![
            video("h264"),
            audio_in("aac", "eng", None, true),
            audio_in("aac", "jpn", None, false),
        ],
        english_subtitles().to_vec(),
    ]
    .concat();
    let japanese_asked = || Facts {
        audio: Some(2),
        subtitle_preference: Some(english_over_foreign_audio()),
        ..facts(Some(tracks.clone()))
    };

    let remuxed = build(&mut japanese_asked());
    let mut no_ffmpeg = japanese_asked();
    without_ffmpeg(&mut no_ffmpeg);
    let dropped = build(&mut no_ffmpeg);

    assert_eq!(
        (remuxed.audio, remuxed.starting_subtitle),
        (Some(2), Some(3))
    );
    assert_eq!(
        (dropped.audio, dropped.starting_subtitle),
        (Some(1), Some(4))
    );
}

#[test]
fn a_shows_subtitle_preference_starts_its_subtitles_over_audio_in_their_language() {
    let [full, _] = english_subtitles();
    let audio = [
        audio_in("aac", "eng", None, true),
        audio_in("aac", "jpn", None, false),
    ];

    assert_eq!(
        starting_subtitle(like(&full), &audio, &english_subtitles()),
        Some(3)
    );
}

/// A stereo audio Track of `codec` in `language`, titled `title`, played by default or not.
fn audio_in(codec: &str, language: &str, title: Option<&str>, default: bool) -> Track {
    Track {
        language: Some(language.to_owned()),
        title: title.map(str::to_owned),
        ..audio(codec, default)
    }
}

/// The place [`audio_asked`] gives for the Audio Preference `preferred` among `tracks`.
fn preferred_audio_among(preferred: Preferred<Description>, tracks: &[Track]) -> Option<usize> {
    let facts = facts(Some(tracks.to_vec()));
    let asked = AskedAudio::Preferred(Some(preferred));
    audio_asked(asked, facts.probe.as_ref(), &facts.device)
}

/// A Show's or Movie's Audio Preference for a Track like `track`.
fn audio_like(track: &Track) -> Preferred<Description> {
    Preferred {
        preference: Description::of(track),
        users_own: false,
        foreign_audio_only: false,
    }
}

#[test]
fn a_preferred_audio_track_must_be_in_its_language_or_none_is_chosen() {
    let japanese = audio_in("aac", "jpn", None, false);
    let tracks = [
        video("h264"),
        audio_in("aac", "eng", None, true),
        audio_in("aac", "ja", None, false),
    ];

    assert_eq!(
        preferred_audio_among(audio_like(&japanese), &tracks),
        Some(2)
    );
    let french = audio_in("aac", "fre", None, false);
    assert_eq!(preferred_audio_among(audio_like(&french), &tracks), None);
    let none = facts(Some(tracks.to_vec()));
    let unset = audio_asked(
        AskedAudio::Preferred(None),
        none.probe.as_ref(),
        &none.device,
    );
    assert_eq!(unset, None);
}

#[test]
fn a_preferred_audio_track_ranks_title_then_one_the_device_decodes_then_the_default_then_the_first()
{
    let wanted = audio_in("aac", "eng", Some("Commentary"), false);
    let cases = [
        (
            "the same title, in any case, beats one the Device decodes",
            vec![
                audio_in("aac", "eng", None, true),
                audio_in("dts", "eng", Some("COMMENTARY"), false),
            ],
            Some(1),
        ),
        (
            "one the Device decodes beats the default",
            vec![
                audio_in("dts", "eng", None, true),
                audio_in("aac", "eng", None, false),
            ],
            Some(1),
        ),
        (
            "the default beats the first",
            vec![
                audio_in("aac", "eng", None, false),
                audio_in("aac", "eng", None, true),
            ],
            Some(1),
        ),
        (
            "a tie goes to the first",
            vec![
                audio_in("aac", "eng", None, false),
                audio_in("aac", "eng", None, false),
            ],
            Some(0),
        ),
    ];

    for (case, tracks, place) in cases {
        assert_eq!(
            preferred_audio_among(audio_like(&wanted), &tracks),
            place,
            "{case}"
        );
    }
}

#[test]
fn the_users_own_audio_preference_skips_the_title_rank() {
    let tracks = [
        audio_in("aac", "eng", Some("Commentary"), false),
        audio_in("dts", "eng", None, true),
        audio_in("aac", "eng", Some("Stereo"), true),
    ];
    let users_own = Preferred {
        preference: Description {
            language: Some("en".to_owned()),
            ..Description::default()
        },
        users_own: true,
        foreign_audio_only: false,
    };
    let untitled = audio_in("aac", "eng", None, false);

    assert_eq!(preferred_audio_among(users_own, &tracks), Some(2));
    assert_eq!(
        preferred_audio_among(audio_like(&untitled), &tracks),
        Some(1),
        "a Show's untitled Preference wants the untitled Track"
    );
}

#[test]
fn a_preferred_audio_track_is_remuxed_or_dropped_as_a_chosen_one_is() {
    let german = audio_in("aac", "ger", None, false);
    let tracks = vec![
        video("h264"),
        audio_in("aac", "eng", None, true),
        german.clone(),
    ];
    let preferred = || {
        let mut facts = facts(Some(tracks.clone()));
        let asked = AskedAudio::Preferred(Some(audio_like(&german)));
        facts.audio = audio_asked(asked, facts.probe.as_ref(), &facts.device);
        facts
    };

    let remuxed = build(&mut preferred());
    let mut no_ffmpeg = preferred();
    without_ffmpeg(&mut no_ffmpeg);
    let dropped = build(&mut no_ffmpeg);

    assert_eq!(remuxed.method(), Some(Method::Remux));
    assert_eq!(remuxed.audio, Some(2));
    assert_eq!(dropped.method(), Some(Method::DirectPlay));
    assert_eq!(dropped.audio, Some(1));
    assert!(reason_of(&dropped, Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED).decided);
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
