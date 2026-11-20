use std::io::Cursor;

use super::{TABLE_LIMIT, keyframes};

/// A box of `kind` holding `body`.
fn mp4_box(kind: [u8; 4], body: &[&[u8]]) -> Vec<u8> {
    let body = body.concat();
    let size = u32::try_from(8 + body.len()).unwrap();
    [&size.to_be_bytes()[..], &kind, &body].concat()
}

/// A full box of `kind` at version 0 without flags, holding `fields` as big-endian `u32`s.
fn full_box(kind: [u8; 4], fields: &[u32]) -> Vec<u8> {
    let body: Vec<u8> = (std::iter::once(&0).chain(fields))
        .flat_map(|field| field.to_be_bytes())
        .collect();
    mp4_box(kind, &[&body])
}

/// A track handled as `handler`, `timescale` units a second, with `edits` as its edit list, each
/// (segment duration, media time), unless there are none, and `tables` in its `stbl`.
fn track(handler: [u8; 4], timescale: u32, edits: &[(u32, i32)], tables: &[Vec<u8>]) -> Vec<u8> {
    let mdhd = full_box(*b"mdhd", &[0, 0, timescale, 0, 0]);
    let hdlr = full_box(*b"hdlr", &[0, u32::from_be_bytes(handler), 0, 0, 0]);
    let stbl = mp4_box(*b"stbl", &[&tables.concat()]);
    let mdia = mp4_box(*b"mdia", &[&mdhd, &hdlr, &mp4_box(*b"minf", &[&stbl])]);
    let edit_box = if edits.is_empty() {
        vec![]
    } else {
        let mut elst = vec![u32::try_from(edits.len()).unwrap()];
        for &(duration, media_time) in edits {
            // A media rate of 1, as 16.16 fixed point.
            elst.extend([duration, media_time.cast_unsigned(), 0x0001_0000]);
        }
        mp4_box(*b"edts", &[&full_box(*b"elst", &elst)])
    };
    mp4_box(*b"trak", &[&edit_box, &mdia])
}

/// A video track 1000 units a second, 8 samples 40 apart, the first and fifth keyframes, with
/// `ctts` if given and `edits` as its edit list.
fn video(edits: &[(u32, i32)], ctts: Option<&[u32]>) -> Vec<u8> {
    let mut tables = vec![
        full_box(*b"stts", &[1, 8, 40]),
        full_box(*b"stss", &[2, 1, 5]),
    ];
    tables.extend(ctts.map(|runs| full_box(*b"ctts", runs)));
    track(*b"vide", 1000, edits, &tables)
}

/// `stts` and `stss` for an audio track, which the reader must pass over for the video.
fn audio() -> Vec<u8> {
    let tables = [
        full_box(*b"stts", &[1, 100, 1024]),
        full_box(*b"stss", &[1, 50]),
    ];
    track(*b"soun", 48000, &[], &tables)
}

/// A `moov` 600 units a second holding `inside`.
fn moov(inside: &[Vec<u8>]) -> Vec<u8> {
    mp4_box(
        *b"moov",
        &[&full_box(*b"mvhd", &[0, 0, 600, 0]), &inside.concat()],
    )
}

/// A file of `ftyp`, `moov` holding an audio track and then `video`, and `mdat`.
fn file(video: Vec<u8>) -> Vec<u8> {
    [
        mp4_box(*b"ftyp", &[b"isom"]),
        moov(&[audio(), video]),
        mp4_box(*b"mdat", &[b"frames"]),
    ]
    .concat()
}

fn read(file: Vec<u8>) -> Option<Vec<i64>> {
    keyframes(&mut Cursor::new(file))
}

#[test]
fn without_ctts_keyframes_fall_at_their_decode_times() {
    let durations = full_box(*b"stts", &[1, 10, 1001]);
    let sync = full_box(*b"stss", &[2, 1, 6]);

    let listed = read(file(track(*b"vide", 24000, &[], &[durations, sync])));

    // The sixth sample decodes at 5 × 1001 / 24000 s, 208.54 ms.
    assert_eq!(listed, Some(vec![0, 208]));
}

#[test]
fn ctts_moves_each_keyframe_to_when_it_shows() {
    let ctts: &[u32] = &[4, 1, 80, 3, 0, 1, 120, 3, 0];

    let listed = read(file(video(&[], Some(ctts))));

    assert_eq!(listed, Some(vec![80, 280]));
}

#[test]
fn an_edit_lists_one_entry_shifts_keyframes_back_by_its_media_time() {
    let ctts: &[u32] = &[4, 1, 80, 3, 0, 1, 120, 3, 0];

    let listed = read(file(video(&[(320, 80)], Some(ctts))));

    assert_eq!(listed, Some(vec![0, 200]));
}

#[test]
fn an_empty_edit_first_delays_keyframes_by_its_length_in_the_movies_units() {
    // 300 of the movie's 600 units a second is half a second.
    let listed = read(file(video(&[(300, -1), (320, 0)], None)));

    assert_eq!(listed, Some(vec![500, 660]));
}

#[test]
fn an_edit_list_of_two_entries_lists_nothing() {
    let listed = read(file(video(&[(160, 0), (160, 160)], None)));

    assert_eq!(listed, None);
}

#[test]
fn a_fragmented_file_lists_nothing() {
    let mvex = mp4_box(*b"mvex", &[&full_box(*b"trex", &[1, 1, 0, 0, 0])]);
    let ftyp = mp4_box(*b"ftyp", &[b"isom"]);
    // An init segment says it's fragmented only by `mvex`; a whole file has `moof`s too.
    let init_segment = [ftyp.clone(), moov(&[video(&[], None), mvex])].concat();
    let fragments_only = [
        ftyp,
        moov(&[video(&[], None)]),
        mp4_box(*b"moof", &[]),
        mp4_box(*b"mdat", &[b"frames"]),
    ]
    .concat();

    assert_eq!(read(init_segment), None);
    assert_eq!(read(fragments_only), None);
}

#[test]
fn a_video_without_stss_lists_nothing() {
    let durations = full_box(*b"stts", &[1, 8, 40]);

    assert_eq!(read(file(track(*b"vide", 1000, &[], &[durations]))), None);
}

#[test]
fn a_truncated_box_lists_nothing() {
    let mut truncated = file(video(&[], None));
    // Cut into `mdat`, and then into `moov`'s last table.
    truncated.truncate(truncated.len() - 20);

    assert_eq!(read(truncated), None);
}

#[test]
fn a_table_past_the_size_limit_lists_nothing() {
    let runs = usize::try_from(TABLE_LIMIT / 8 + 1).unwrap();
    let mut runs_table = vec![u32::try_from(runs).unwrap()];
    runs_table.extend(std::iter::repeat_n([1, 40], runs).flatten());
    let sync = full_box(*b"stss", &[1, 1]);

    let listed = read(file(track(
        *b"vide",
        1000,
        &[],
        &[full_box(*b"stts", &runs_table), sync],
    )));

    assert_eq!(listed, None);
}

#[test]
fn a_moov_after_a_64_bit_mdat_is_read_the_same() {
    let frames = b"frames";
    let mdat_size = 16 + u64::try_from(frames.len()).unwrap();
    let mdat = [
        &1_u32.to_be_bytes()[..],
        b"mdat",
        &mdat_size.to_be_bytes(),
        frames,
    ]
    .concat();
    let moov_last = [
        mp4_box(*b"ftyp", &[b"isom"]),
        mdat,
        moov(&[audio(), video(&[], None)]),
    ]
    .concat();

    assert_eq!(read(moov_last), Some(vec![0, 160]));
}
