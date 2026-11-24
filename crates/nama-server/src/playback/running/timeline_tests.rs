use super::Timeline;

#[test]
fn segments_are_cut_at_the_first_keyframe_past_each_six_seconds() {
    let every_two_and_a_half_seconds: Vec<i64> = (0..12).map(|n| n * 2500).collect();

    let timeline = Timeline::new(every_two_and_a_half_seconds, 30_020);

    let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
    assert_eq!(segment_ms, [7500, 5000, 7500, 5000, 5020]);
    assert_eq!(timeline.keyframes_of(1), 3..5);
    assert_eq!(timeline.keyframes_of(4), 10..12);
}

#[test]
fn a_keyframe_at_the_length_joins_the_last_segment() {
    let keyframes: Vec<i64> = (0..4).map(|n| n * 2000).collect();

    let timeline = Timeline::new(keyframes, 6000);

    let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
    assert_eq!(segment_ms, [6000]);
    assert_eq!(timeline.keyframes_of(0), 0..4);
    assert!(!timeline.playlist().contains("#EXTINF:0.000"));
}

#[test]
fn a_transcode_lists_a_segment_for_each_six_seconds_of_its_length() {
    let playlist = Timeline::transcoded(13_999).playlist();

    let listed: Vec<&str> = (playlist.lines())
        .filter(|line| line.starts_with("#EXT-X-TARGETDURATION") || line.starts_with("#EXTINF"))
        .collect();
    assert_eq!(
        listed,
        [
            "#EXT-X-TARGETDURATION:6",
            "#EXTINF:6.000,",
            "#EXTINF:6.000,",
            "#EXTINF:1.999,",
        ]
    );
}

#[test]
fn a_position_is_in_the_last_segment_starting_at_or_before_it() {
    // Segments start at 0, 7500, 12 500, 20 000 and 25 000 ms.
    let timeline = Timeline::new((0..12).map(|n| n * 2500).collect(), 30_020);

    assert_eq!(timeline.segment_at(0), 0);
    assert_eq!(timeline.segment_at(10_000), 1);
    assert_eq!(timeline.segment_at(12_500), 2);
    assert_eq!(timeline.segment_at(40_000), 4);
}
