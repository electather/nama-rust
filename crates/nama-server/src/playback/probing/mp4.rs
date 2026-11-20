//! Reading an MP4's keyframes from its sample tables, private to Probing (ADR 0066). It hops from
//! box header to box header and reads only the first video track's tables, so a `moov` after
//! `mdat` costs no more than one before it.

use std::{
    io::{Read, Seek, SeekFrom},
    ops::Range,
};

/// The most bytes read of any one box, generous next to a three-hour 60 fps video's `stts` or
/// `ctts` of about 5 MB.
const TABLE_LIMIT: u64 = 16 << 20;

/// The most boxes read inside any one box, or at the top of the file, far past the few a file
/// has, so a malformed one of countless tiny boxes isn't read without limit.
const BOXES_LIMIT: usize = 1024;

/// The most bytes of track ids read of any one `chap`, 64 of them, far past the chapter tracks a
/// file has, as every track may have one.
const CHAPTER_IDS_LIMIT: u64 = 4 * 64;

/// An edit's media rate of 1, in 16.16 fixed point.
const NORMAL_RATE: u32 = 0x0001_0000;

struct Mp4Box {
    kind: [u8; 4],
    contents: Range<u64>,
}

/// When each keyframe of the file's first video shows, in milliseconds, as ffmpeg times its
/// packets: its decode time from `stts`, plus its `ctts` offset, less its edit list's start.
/// `None` if they aren't listed as this reads them, as in a fragmented file, one without `stss`,
/// or with more than one non-empty edit, a box past [`TABLE_LIMIT`], or anything malformed.
pub(super) fn keyframes(file: &mut (impl Read + Seek)) -> Option<Vec<i64>> {
    let end = file.seek(SeekFrom::End(0)).ok()?;
    let top = children(file, 0..end)?;
    let moov = children(file, find(&top, *b"moov")?.contents.clone())?;
    // Fragmented: each fragment's keyframes are in its own `moof`.
    if find(&moov, *b"mvex").is_some() {
        return None;
    }
    let movie_timescale = timescale(&read(file, find(&moov, *b"mvhd")?)?)?;
    let (trak, mdia) = first_video(file, &moov)?;
    let timescale = timescale(&read(file, find(&mdia, *b"mdhd")?)?)?;
    let start = edit_start(file, &trak, timescale, movie_timescale)?;
    let minf = children(file, find(&mdia, *b"minf")?.contents.clone())?;
    let stbl = children(file, find(&minf, *b"stbl")?.contents.clone())?;
    let shown = shown_at(file, &stbl)?;
    shown
        .into_iter()
        .map(|at| millis(at - start, timescale))
        .collect()
}

/// When each keyframe the sample table `stbl` lists shows, in its track's units, before its edit
/// list moves it.
fn shown_at(file: &mut (impl Read + Seek), stbl: &[Mp4Box]) -> Option<Vec<i128>> {
    let samples = sync_samples(&read(file, find(stbl, *b"stss")?)?)?;
    let stts = runs(&read(file, find(stbl, *b"stts")?)?, i64::from)?;
    let decoded = in_runs(&stts, &samples)?;
    let offsets = match find(stbl, *b"ctts") {
        Some(ctts) => {
            // ffmpeg reads every offset as signed, whatever the table's version says.
            let ctts = runs(&read(file, ctts)?, |offset| i64::from(offset.cast_signed()))?;
            in_runs(&ctts, &samples)?
        }
        None => vec![(0, 0); samples.len()],
    };
    let shown = (decoded.into_iter().zip(offsets))
        .map(|((decoded, _), (_, offset))| decoded + i128::from(offset));
    Some(shown.collect())
}

/// The boxes of the first track in `moov` that `hdlr` says is video, and of its `mdia`, passing
/// over the chapter pictures [`chapter_tracks`] names.
fn first_video(
    file: &mut (impl Read + Seek),
    moov: &[Mp4Box],
) -> Option<(Vec<Mp4Box>, Vec<Mp4Box>)> {
    let chapters = chapter_tracks(file, moov)?;
    for trak in moov.iter().filter(|child| &child.kind == b"trak") {
        let trak = children(file, trak.contents.clone())?;
        let mdia = children(file, find(&trak, *b"mdia")?.contents.clone())?;
        // After `hdlr`'s version, flags and a field always 0.
        let handler = read_head(file, find(&mdia, *b"hdlr")?, 12)?;
        if handler.get(8..12) != Some(b"vide") {
            continue;
        }
        // `tkhd`'s id sits where `mdhd`'s timescale does.
        let pictures = !chapters.is_empty()
            && chapters.contains(&after_times(&read_head(file, find(&trak, *b"tkhd")?, 24)?)?);
        if !pictures {
            return Some((trak, mdia));
        }
    }
    None
}

/// The ids of the tracks a `tref`'s `chap` in `moov` names. ffmpeg reads a video one as chapter
/// pictures, not as the file's video, so a whole-file listing passes over it.
fn chapter_tracks(file: &mut (impl Read + Seek), moov: &[Mp4Box]) -> Option<Vec<u32>> {
    let mut chapters = vec![];
    for trak in moov.iter().filter(|child| &child.kind == b"trak") {
        let trak = children(file, trak.contents.clone())?;
        let Some(tref) = find(&trak, *b"tref") else {
            continue;
        };
        let tref = children(file, tref.contents.clone())?;
        let Some(chap) = find(&tref, *b"chap") else {
            continue;
        };
        if chap.contents.end - chap.contents.start > CHAPTER_IDS_LIMIT {
            return None;
        }
        let chap = read(file, chap)?;
        let (ids, _) = chap.as_chunks::<4>();
        chapters.extend(ids.iter().map(|&id| u32::from_be_bytes(id)));
    }
    Some(chapters)
}

/// Where the track's edit list in `trak` starts it, in its `timescale`'s units: the media time of
/// its one non-empty edit, less the length of an empty edit before it, which is in the movie's
/// `movie_timescale`. 0 without an edit list.
fn edit_start(
    file: &mut (impl Read + Seek),
    trak: &[Mp4Box],
    timescale: u32,
    movie_timescale: u32,
) -> Option<i128> {
    let Some(edts) = find(trak, *b"edts") else {
        return Some(0);
    };
    let edts = children(file, edts.contents.clone())?;
    let Some(elst) = find(&edts, *b"elst") else {
        return Some(0);
    };
    let (empty, media_time, rate) = match *edits(&read(file, elst)?)?.as_slice() {
        [(_, media_time, rate)] => (0, media_time, rate),
        [(empty, -1, _), (_, media_time, rate)] => (empty, media_time, rate),
        _ => return None,
    };
    if media_time < 0 || rate != NORMAL_RATE {
        return None;
    }
    // ponytail: ffmpeg also shifts a track with an edit list so its earliest sample shows at 0,
    // which here would mean reading every sample's time; an edit whose media time falls short of
    // that, such as 0 for a video with B-frames, fails the check instead and the file is read
    // whole, as before. Mirror it if such files turn up often.
    // Rounded to the nearest unit, as ffmpeg rescales it.
    let empty = (2 * i128::from(empty) * i128::from(timescale) + i128::from(movie_timescale))
        / (2 * i128::from(movie_timescale));
    Some(i128::from(media_time) - empty)
}

/// The boxes inside `within`, in order. A `moof` among them means the file is fragmented.
fn children(file: &mut (impl Read + Seek), within: Range<u64>) -> Option<Vec<Mp4Box>> {
    let mut boxes = vec![];
    let mut at = within.start;
    while at < within.end && boxes.len() < BOXES_LIMIT {
        let child = header(file, at, within.end)?;
        if &child.kind == b"moof" {
            return None;
        }
        at = child.contents.end;
        boxes.push(child);
    }
    (at >= within.end).then_some(boxes)
}

/// The box whose header starts at `at`, which must end by `end`.
fn header(file: &mut (impl Read + Seek), at: u64, end: u64) -> Option<Mp4Box> {
    file.seek(SeekFrom::Start(at)).ok()?;
    let mut head = [0; 8];
    file.read_exact(&mut head).ok()?;
    let [s0, s1, s2, s3, k0, k1, k2, k3] = head;
    let (contents_start, size) = match u32::from_be_bytes([s0, s1, s2, s3]) {
        // A size of 1 says a 64-bit size follows the type; 0, that the box runs to the end.
        1 => {
            let mut size = [0; 8];
            file.read_exact(&mut size).ok()?;
            (at + 16, u64::from_be_bytes(size))
        }
        0 => (at + 8, end - at),
        size => (at + 8, u64::from(size)),
    };
    let box_end =
        (at.checked_add(size)).filter(|box_end| (contents_start..=end).contains(box_end))?;
    Some(Mp4Box {
        kind: [k0, k1, k2, k3],
        contents: contents_start..box_end,
    })
}

fn find(boxes: &[Mp4Box], kind: [u8; 4]) -> Option<&Mp4Box> {
    boxes.iter().find(|candidate| candidate.kind == kind)
}

/// `table`'s contents, unless they're past [`TABLE_LIMIT`].
fn read(file: &mut (impl Read + Seek), table: &Mp4Box) -> Option<Vec<u8>> {
    let length = table.contents.end - table.contents.start;
    if length > TABLE_LIMIT {
        return None;
    }
    read_head(file, table, length)
}

/// The first `length` bytes of `of`'s contents, or all of them if it holds fewer.
fn read_head(file: &mut (impl Read + Seek), of: &Mp4Box, length: u64) -> Option<Vec<u8>> {
    let length = length.min(of.contents.end - of.contents.start);
    file.seek(SeekFrom::Start(of.contents.start)).ok()?;
    let mut contents = vec![0; usize::try_from(length).ok()?];
    file.read_exact(&mut contents).ok()?;
    Some(contents)
}

/// The timescale in an `mvhd` or `mdhd`, its units a second.
fn timescale(header: &[u8]) -> Option<u32> {
    after_times(header).filter(|&timescale| timescale > 0)
}

/// The field of an `mvhd`, `mdhd` or `tkhd` after its version, flags, and its creation and
/// modification times, which are 64-bit at version 1.
fn after_times(header: &[u8]) -> Option<u32> {
    be_u32(header, if *header.first()? == 1 { 20 } else { 12 })
}

/// The keyframes `stss` lists, as samples counted from 0, in increasing order.
fn sync_samples(stss: &[u8]) -> Option<Vec<u32>> {
    let samples: Vec<u32> = (entries::<4>(stss)?.iter())
        .map(|&number| u32::from_be_bytes(number).checked_sub(1))
        .collect::<Option<_>>()?;
    samples.is_sorted_by(|a, b| a < b).then_some(samples)
}

/// The runs of an `stts` or `ctts`: how many samples, and the value each of them has, read by
/// `value`.
fn runs(table: &[u8], value: fn(u32) -> i64) -> Option<Vec<(u32, i64)>> {
    (entries::<8>(table)?.iter())
        .map(|run| Some((be_u32(run, 0)?, value(be_u32(run, 4)?))))
        .collect()
}

/// For each of `samples`, counted from 0 and in increasing order, the sum of the values `runs`
/// give the samples before it, and its own value. `None` if one falls past the runs.
fn in_runs(runs: &[(u32, i64)], samples: &[u32]) -> Option<Vec<(i128, i64)>> {
    let mut runs = runs.iter();
    let &(mut count, mut value) = runs.next()?;
    let (mut run_start, mut sum_before_run) = (0_u64, 0_i128);
    let mut placed = Vec::with_capacity(samples.len());
    for &sample in samples {
        let sample = u64::from(sample);
        while sample >= run_start + u64::from(count) {
            run_start += u64::from(count);
            sum_before_run += i128::from(count) * i128::from(value);
            (count, value) = *runs.next()?;
        }
        placed.push((
            sum_before_run + i128::from(sample - run_start) * i128::from(value),
            value,
        ));
    }
    Some(placed)
}

/// The entries of an `elst`: each segment's duration, its media time, -1 for an empty edit, and
/// its media rate.
fn edits(elst: &[u8]) -> Option<Vec<(u64, i64, u32)>> {
    // Version 1's durations and times are 64-bit.
    if *elst.first()? == 1 {
        (entries::<20>(elst)?.iter())
            .map(|edit| {
                let duration = u64::from_be_bytes(bytes(edit, 0)?);
                let media_time = i64::from_be_bytes(bytes(edit, 8)?);
                Some((duration, media_time, be_u32(edit, 16)?))
            })
            .collect()
    } else {
        (entries::<12>(elst)?.iter())
            .map(|edit| {
                let media_time = i32::from_be_bytes(bytes(edit, 4)?);
                Some((
                    u64::from(be_u32(edit, 0)?),
                    i64::from(media_time),
                    be_u32(edit, 8)?,
                ))
            })
            .collect()
    }
}

/// The `N`-byte entries of a full box's table, after its version, flags and how many there are.
fn entries<const N: usize>(table: &[u8]) -> Option<&[[u8; N]]> {
    let count = usize::try_from(be_u32(table, 4)?).ok()?;
    let (entries, _) = table.get(8..)?.as_chunks::<N>();
    entries.get(..count)
}

fn bytes<const N: usize>(data: &[u8], at: usize) -> Option<[u8; N]> {
    data.get(at..)?.first_chunk().copied()
}

fn be_u32(data: &[u8], at: usize) -> Option<u32> {
    bytes(data, at).map(u32::from_be_bytes)
}

/// `time` in `timescale` units a second, in whole milliseconds rounded down.
fn millis(time: i128, timescale: u32) -> Option<i64> {
    i64::try_from((time * 1000).div_euclid(i128::from(timescale))).ok()
}

#[cfg(test)]
#[path = "mp4_tests.rs"]
mod tests;
