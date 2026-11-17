use super::*;

/// What the name of the sidecar `name` says beside `Movie.mkv`: its language, forced, SDH and
/// title.
fn named(name: &str) -> (Option<String>, bool, bool, Option<String>) {
    let track = track_named("Movie", name).unwrap();
    (track.language, track.forced, track.sdh, track.title)
}

#[test]
fn a_sidecars_name_says_its_language_flags_and_title() {
    assert_eq!(
        named("Movie.en.srt"),
        (Some("en".into()), false, false, None)
    );
    assert_eq!(
        named("Movie.eng.sdh.srt"),
        (Some("eng".into()), false, true, None)
    );
    assert_eq!(
        named("Movie.en.forced.srt"),
        (Some("en".into()), true, false, None)
    );
    assert_eq!(
        named("Movie.en.hi.srt"),
        (Some("en".into()), false, true, None)
    );
    assert_eq!(
        named("Movie.pt-BR.srt"),
        (Some("pt-BR".into()), false, false, None)
    );
    assert_eq!(named("Movie.srt"), (None, false, false, None));
    assert_eq!(
        named("Movie.en.Commentary.srt"),
        (
            Some("en".into()),
            false,
            false,
            Some("Commentary".to_owned())
        )
    );
}

#[test]
fn hi_alone_is_hindi_not_sdh() {
    assert_eq!(
        named("Movie.hi.srt"),
        (Some("hi".into()), false, false, None)
    );
}

#[test]
fn a_sidecars_flags_and_extension_are_read_in_any_case() {
    let track = track_named("MOVIE", "MOVIE.EN.FORCED.SDH.DEFAULT.SRT").unwrap();

    assert_eq!(track.language.as_deref(), Some("EN"));
    assert!(track.forced && track.sdh && track.default);
    assert_eq!(track.codec, "subrip");
}

#[test]
fn a_sidecars_codec_is_ffprobes_name_for_its_extension() {
    let codec = |name| track_named("Movie", name).map(|track| track.codec);

    assert_eq!(codec("Movie.srt").as_deref(), Some("subrip"));
    assert_eq!(codec("Movie.vtt").as_deref(), Some("webvtt"));
    assert_eq!(codec("Movie.ass").as_deref(), Some("ass"));
    assert_eq!(codec("Movie.ssa").as_deref(), Some("ssa"));
    assert_eq!(codec("Movie.sup").as_deref(), Some("hdmv_pgs_subtitle"));
    assert_eq!(codec("Movie.nfo"), None);
}

#[test]
fn only_an_image_sidecar_is_an_image_subtitle() {
    let image = |name| track_named("Movie", name).unwrap().kind;

    assert_eq!(
        image("Movie.sup"),
        TrackKind::Subtitle {
            image: true,
            header: false,
            sidecar: None,
        }
    );
    assert_eq!(
        image("Movie.srt"),
        TrackKind::Subtitle {
            image: false,
            header: false,
            sidecar: None,
        }
    );
}
