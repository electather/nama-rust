use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use super::{audio_asked, preferred_subtitle};
use crate::playback::{
    AskedAudio, DeviceDecoding, Facts, Method, build,
    tests::{audio, facts, firefox, reason_of, track, video, without_ffmpeg},
    track_preferences::{Description, Preferred, SubtitlePreference},
    tracks::{Track, TrackKind},
};

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
