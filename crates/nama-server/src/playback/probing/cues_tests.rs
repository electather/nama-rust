use std::io::Cursor;

use super::{
    CUE_POINT, CUE_TIME, CUE_TRACK, CUE_TRACK_POSITIONS, CUES, EBML, ELEMENT_LIMIT, INFO, SEEK,
    SEEK_HEAD, SEEK_ID, SEEK_POSITION, SEGMENT, TIMESTAMP_SCALE, TRACK_ENTRY, TRACK_NUMBER,
    TRACK_TYPE, TRACKS, keyframes,
};

/// EBML's Void element, which readers skip.
const VOID: u32 = 0xEC;

const VIDEO: u64 = 1;
const AUDIO: u64 = 2;

/// The element `id` holding `body`, its size written in 8 bytes so a file's layout doesn't shift
/// as the positions in it change.
fn element(id: u32, body: &[u8]) -> Vec<u8> {
    let id = id.to_be_bytes();
    let id = &id[id.iter().position(|&byte| byte != 0).unwrap()..];
    let size = (body.len() as u64 | 1 << 56).to_be_bytes();
    [id, &size, body].concat()
}

fn uint(id: u32, value: u64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

fn info(timestamp_scale: u64) -> Vec<u8> {
    element(INFO, &uint(TIMESTAMP_SCALE, timestamp_scale))
}

/// Tracks of the given numbers and types.
fn tracks(entries: &[(u64, u64)]) -> Vec<u8> {
    let entries = entries.iter().map(|&(number, kind)| {
        element(
            TRACK_ENTRY,
            &[uint(TRACK_NUMBER, number), uint(TRACK_TYPE, kind)].concat(),
        )
    });
    element(TRACKS, &entries.collect::<Vec<_>>().concat())
}

/// Cue points at the given times, each for the given track.
fn cues(points: &[(u64, u64)]) -> Vec<u8> {
    let points = points.iter().map(|&(time, track)| {
        let positions = element(CUE_TRACK_POSITIONS, &uint(CUE_TRACK, track));
        element(CUE_POINT, &[uint(CUE_TIME, time), positions].concat())
    });
    element(CUES, &points.collect::<Vec<_>>().concat())
}

/// A `SeekHead` listing each of `listed`, an element id and its position in the Segment.
fn seek_head(listed: &[(u32, u64)]) -> Vec<u8> {
    let seeks = listed.iter().map(|&(id, position)| {
        let id = element(
            SEEK_ID,
            &id.to_be_bytes()[id.leading_zeros() as usize / 8..],
        );
        element(SEEK, &[id, uint(SEEK_POSITION, position)].concat())
    });
    element(SEEK_HEAD, &seeks.collect::<Vec<_>>().concat())
}

/// A Matroska file holding `elements` in its Segment, after a `SeekHead` listing those of them
/// `first_lists` says. A second `SeekHead` at the end lists the rest, if any, and the first lists
/// it.
fn matroska(elements: &[Vec<u8>], first_lists: usize) -> Vec<u8> {
    let id_of = |element: &[u8]| {
        let length = element[0].leading_zeros() as usize + 1;
        element[..length]
            .iter()
            .fold(0, |id, &byte| id << 8 | u32::from(byte))
    };
    let mut positions = vec![];
    let second_lists = elements.len() - first_lists;
    let first_entries = first_lists + usize::from(second_lists > 0);
    // Each entry is the same size, whatever position it holds.
    let mut at = seek_head(&vec![(CUES, 0); first_entries]).len() as u64;
    for element in elements {
        positions.push((id_of(element), at));
        at += element.len() as u64;
    }
    let (first, rest) = positions.split_at(first_lists);
    let mut first = first.to_vec();
    let mut segment = vec![];
    if !rest.is_empty() {
        first.push((SEEK_HEAD, at));
    }
    segment.extend(seek_head(&first));
    segment.extend(elements.concat());
    if !rest.is_empty() {
        segment.extend(seek_head(rest));
    }
    [element(EBML, b"matroska"), element(SEGMENT, &segment)].concat()
}

/// What the Cues of `file`, starting at 0 and 60 s long, list.
fn listed(file: Vec<u8>) -> Option<Vec<i64>> {
    keyframes(&mut Cursor::new(file), 0, Some(60_000)).map(|listed| listed.keyframes_ms)
}

fn video_and_audio() -> Vec<u8> {
    tracks(&[(1, VIDEO), (2, AUDIO)])
}

#[test]
fn keyframes_are_read_from_cues_the_first_seek_head_lists() {
    let file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(0, 1), (2_000, 1), (4_000, 1)]),
        ],
        3,
    );

    assert_eq!(listed(file), Some(vec![0, 2_000, 4_000]));
}

#[test]
fn keyframes_are_read_from_cues_a_second_seek_head_lists() {
    let file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(0, 1), (2_000, 1)]),
        ],
        2,
    );

    assert_eq!(listed(file), Some(vec![0, 2_000]));
}

#[test]
fn a_file_without_cues_lists_none() {
    let file = matroska(&[info(1_000_000), video_and_audio()], 2);

    assert_eq!(listed(file), None);
}

#[test]
fn cue_points_out_of_order_list_none() {
    let file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(0, 1), (4_000, 1), (2_000, 1)]),
        ],
        3,
    );

    assert_eq!(listed(file), None);
}

#[test]
fn cues_not_starting_at_the_files_start_list_none() {
    let file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(2_000, 1), (4_000, 1)]),
        ],
        3,
    );

    assert_eq!(listed(file), None);
}

#[test]
fn cues_ending_past_the_versions_length_list_none() {
    let file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(0, 1), (61_000, 1)]),
        ],
        3,
    );

    assert_eq!(listed(file), None);
}

#[test]
fn cues_are_timed_from_the_files_start() {
    let file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(42, 1), (2_042, 1)]),
        ],
        3,
    );

    let listed = keyframes(&mut Cursor::new(file), 42, Some(60_000)).unwrap();

    assert_eq!(
        (listed.file_start_ms, listed.keyframes_ms),
        (42, vec![0, 2_000])
    );
}

#[test]
fn only_the_first_video_tracks_cue_points_are_keyframes() {
    let file = matroska(
        &[
            info(1_000_000),
            tracks(&[(1, AUDIO), (2, VIDEO), (3, VIDEO)]),
            cues(&[(0, 1), (0, 2), (1_000, 1), (1_000, 3), (2_000, 2)]),
        ],
        3,
    );

    assert_eq!(listed(file), Some(vec![0, 2_000]));
}

#[test]
fn cue_times_are_scaled_by_the_timestamp_scale() {
    // Ticks of 100 µs.
    let file = matroska(
        &[
            info(100_000),
            video_and_audio(),
            cues(&[(0, 1), (20_015, 1)]),
        ],
        3,
    );

    assert_eq!(listed(file), Some(vec![0, 2_001]));
}

#[test]
fn a_truncated_element_lists_none() {
    let mut file = matroska(
        &[
            info(1_000_000),
            video_and_audio(),
            cues(&[(0, 1), (2_000, 1)]),
        ],
        3,
    );
    file.truncate(file.len() - 1);

    assert_eq!(listed(file), None);
}

#[test]
fn an_element_past_the_size_limit_lists_none() {
    let padding = element(VOID, &vec![0; usize::try_from(ELEMENT_LIMIT).unwrap()]);
    let cues = cues(&[(0, 1), (2_000, 1)]);
    let padded = element(CUES, &[&cues[12..], &padding].concat());
    let file = matroska(&[info(1_000_000), video_and_audio(), padded], 3);

    assert_eq!(listed(file), None);
}

#[test]
fn a_file_that_isnt_matroska_lists_none() {
    assert_eq!(listed(vec![0; 64]), None);
}
