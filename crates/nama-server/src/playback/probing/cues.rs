//! Reading a Matroska file's keyframes from its Cues, the index of them it usually carries,
//! private to Probing. It reads only the Segment's `SeekHead`s, Info, Tracks and Cues, never its
//! Clusters, so it takes a fraction of a second even over a network share (#1122).

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};

use super::plausible;
use crate::playback::tracks::ListedKeyframes;

// Element ids, as the EBML and Matroska specifications number them.
const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;

/// The `TrackType` of a video track.
const VIDEO: u64 = 1;

/// Nanoseconds per tick of a file whose Info doesn't say: a millisecond.
const DEFAULT_TIMESTAMP_SCALE: u64 = 1_000_000;

/// The most bytes one element read may hold, about ten times the largest Cues seen (867 KB), so
/// a malformed or hostile file can't make Probing read without limit.
const ELEMENT_LIMIT: u64 = 8 << 20;

/// The keyframes the Cues of the Matroska file at `path` list, as [`keyframes`] reads them.
pub(super) async fn listed(
    path: &str,
    file_start_ms: i64,
    duration_ms: Option<i64>,
) -> Option<ListedKeyframes> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut file = BufReader::new(File::open(path).ok()?);
        keyframes(&mut file, file_start_ms, duration_ms)
    })
    .await
    .ok()
    .flatten()
}

/// When each keyframe of the first video in the Matroska `file` falls, in milliseconds from
/// `file_start_ms`, as its Cues list them. `None` if they don't, if anything on the way is
/// malformed or past [`ELEMENT_LIMIT`], or if the list isn't [`plausible`].
pub(super) fn keyframes(
    file: &mut (impl Read + Seek),
    file_start_ms: i64,
    duration_ms: Option<i64>,
) -> Option<ListedKeyframes> {
    let segment = segment_start(file)?;
    let mut positions = seek_entries(&read_element(file, segment, SEEK_HEAD)?)?;
    // Some muxers list their Cues only in a second `SeekHead`, which the first lists.
    if !positions.contains_key(&CUES)
        && let Some(&second) = positions.get(&SEEK_HEAD)
    {
        let second = read_element(file, segment.checked_add(second)?, SEEK_HEAD)?;
        positions.extend(seek_entries(&second)?);
    }
    let mut listed_element =
        |id| read_element(file, segment.checked_add(*positions.get(&id)?)?, id);
    let scale = timestamp_scale(&listed_element(INFO)?)?;
    let video = first_video_track(&listed_element(TRACKS)?)?;
    let keyframes_ms = cue_times(&listed_element(CUES)?, video)?
        .into_iter()
        .map(|ticks| {
            let ms = u128::from(ticks) * u128::from(scale) / 1_000_000;
            Some(i64::try_from(ms).ok()? - file_start_ms)
        })
        .collect::<Option<_>>()?;
    let listed = ListedKeyframes {
        file_start_ms,
        keyframes_ms,
    };
    plausible(&listed, 0, duration_ms).then_some(listed)
}

/// Where the data of `file`'s Segment starts, after its EBML header: what every `SeekPosition`
/// counts from.
fn segment_start(file: &mut (impl Read + Seek)) -> Option<u64> {
    file.seek(SeekFrom::Start(0)).ok()?;
    let (EBML, Some(size)) = header(file)? else {
        return None;
    };
    file.seek(SeekFrom::Current(i64::try_from(size).ok()?))
        .ok()?;
    // Its size isn't needed, and is unknown in a file written live.
    let (SEGMENT, _) = header(file)? else {
        return None;
    };
    file.stream_position().ok()
}

/// The data of the element `id` at `position` in `file`, or `None` if another element is there,
/// its size is unknown or past [`ELEMENT_LIMIT`], or the file ends first.
fn read_element(file: &mut (impl Read + Seek), position: u64, id: u32) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(position)).ok()?;
    let (found, Some(size)) = header(file)? else {
        return None;
    };
    if found != id || size > ELEMENT_LIMIT {
        return None;
    }
    let mut data = vec![0; usize::try_from(size).ok()?];
    file.read_exact(&mut data).ok()?;
    Some(data)
}

/// The id and size of the element `file` is at, leaving it at the element's data. The size is
/// `None` when unknown.
fn header(file: &mut impl Read) -> Option<(u32, Option<u64>)> {
    let (id_length, id) = read_vint(file)?;
    let (size_length, marked_size) = read_vint(file)?;
    // An id is at most 4 bytes.
    if id_length > 4 {
        return None;
    }
    let marker = 1 << (7 * size_length);
    let size = marked_size ^ marker;
    // A size of all ones is unknown.
    Some((
        u32::try_from(id).ok()?,
        (size != marker - 1).then_some(size),
    ))
}

/// The variable-length integer `file` is at, as its length in bytes and its value with the
/// length's marker bit kept, as an element id keeps it.
fn read_vint(file: &mut impl Read) -> Option<(u32, u64)> {
    let mut bytes = [0; 8];
    let first = bytes.get_mut(..1)?;
    file.read_exact(first).ok()?;
    let length = first.first()?.leading_zeros() + 1;
    let bytes = bytes.get_mut(..usize::try_from(length).ok()?)?;
    file.read_exact(bytes.get_mut(1..)?).ok()?;
    Some((length, uint(bytes)?))
}

/// The child elements in `data`, each as its id and data, or `None` if one doesn't say its size
/// or runs past the end.
fn children(mut data: &[u8]) -> Option<Vec<(u32, &[u8])>> {
    let mut children = vec![];
    while !data.is_empty() {
        let (id, size) = header(&mut data)?;
        let (child, rest) = data.split_at_checked(usize::try_from(size?).ok()?)?;
        children.push((id, child));
        data = rest;
    }
    Some(children)
}

/// The data of the first of `children` with the id `id`.
fn child<'a>(children: &[(u32, &'a [u8])], id: u32) -> Option<&'a [u8]> {
    (children.iter()).find_map(|&(child, data)| (child == id).then_some(data))
}

/// An unsigned integer element's `data`, big-endian in up to 8 bytes.
fn uint(data: &[u8]) -> Option<u64> {
    (data.len() <= 8).then(|| (data.iter()).fold(0, |value, &byte| value << 8 | u64::from(byte)))
}

/// Where each element a `SeekHead`'s `data` lists is, by id, from the Segment's data start.
fn seek_entries(data: &[u8]) -> Option<HashMap<u32, u64>> {
    (children(data)?.into_iter())
        .filter(|&(id, _)| id == SEEK)
        .map(|(_, seek)| {
            let seek = children(seek)?;
            let id = u32::try_from(uint(child(&seek, SEEK_ID)?)?).ok()?;
            Some((id, uint(child(&seek, SEEK_POSITION)?)?))
        })
        .collect()
}

/// Nanoseconds per tick, as Info's `data` says.
fn timestamp_scale(info: &[u8]) -> Option<u64> {
    child(&children(info)?, TIMESTAMP_SCALE).map_or(Some(DEFAULT_TIMESTAMP_SCALE), uint)
}

/// The number of the first video track in Tracks' `data`.
fn first_video_track(tracks: &[u8]) -> Option<u64> {
    for (id, entry) in children(tracks)? {
        if id != TRACK_ENTRY {
            continue;
        }
        let entry = children(entry)?;
        if uint(child(&entry, TRACK_TYPE)?)? == VIDEO {
            return uint(child(&entry, TRACK_NUMBER)?);
        }
    }
    None
}

/// When each cue point in Cues' `data` for the track `track` falls, in ticks.
fn cue_times(cues: &[u8], track: u64) -> Option<Vec<u64>> {
    let mut times = vec![];
    for (id, point) in children(cues)? {
        if id != CUE_POINT {
            continue;
        }
        let point = children(point)?;
        let tracks: Vec<u64> = (point.iter())
            .filter(|&&(child, _)| child == CUE_TRACK_POSITIONS)
            .map(|&(_, positions)| uint(child(&children(positions)?, CUE_TRACK)?))
            .collect::<Option<_>>()?;
        if tracks.contains(&track) {
            times.push(uint(child(&point, CUE_TIME)?)?);
        }
    }
    Some(times)
}

#[cfg(test)]
#[path = "cues_tests.rs"]
mod tests;
