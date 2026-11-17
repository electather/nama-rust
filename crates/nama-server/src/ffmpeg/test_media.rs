//! Writing short media files with the ffmpeg on `PATH`, for tests to probe, Remux and Transcode.

use std::{io, path::Path, process::Command};

/// A cue of Persian, which older `.srt` files hold in Windows-1256. Its yeh is the Arabic one
/// they write, as Windows-1256 has no Farsi yeh.
#[cfg(any(test, feature = "testing"))]
pub const PERSIAN_SRT: &str =
    "1\n00:00:01,000 --> 00:00:03,000\nسلام، امروز هوا خيلي خوب است و ما به پارک مي‌رويم.\n";

/// A cue of French with `é` and `ü`, which older `.srt` files hold in Windows-1252.
#[cfg(any(test, feature = "testing"))]
pub const FRENCH_SRT: &str = "1\n00:00:01,000 --> 00:00:03,000\nLe café de Saül est très bon, mais sa cuisine est un vrai capharnaüm.\n";

/// Writes a fifth of a second of 10-bit 4K video with English 5.1 audio, as Matroska, to `path`,
/// with the ffmpeg on `PATH`, for a test to probe. The video is tagged with `transfer`, as ffmpeg
/// names it: `smpte2084` for HDR10, `arib-std-b67` for HLG or `bt709` for SDR.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_4k_with_5_1(path: &Path, transfer: &str) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        // The frames carry the colours, since ffmpeg 9 drops `-color_trc` and the like for FFV1.
        let picture = format!(
            "color=size=3840x2160:rate=24,format=yuv420p10le,\
             setparams=color_primaries=bt2020:color_trc={transfer}:colorspace=bt2020nc"
        );
        ffmpeg
            .args(["-f", "lavfi", "-i", &picture])
            .args(["-f", "lavfi", "-i", "anullsrc=channel_layout=5.1"])
            // Encoders every ffmpeg build has; of those, FFV1 is one that takes 10 bits.
            .args(["-t", "0.2", "-c:v", "ffv1", "-pix_fmt", "yuv420p10le", "-c:a", "ac3"])
            .args(["-metadata:s:a:0", "language=eng", "-f", "matroska"])
    })
}

/// Writes 14 seconds of H.264 video with B-frames and a keyframe every 2 seconds, and 5.1 audio
/// encoded with `audio`, such as `aac` or `dca` for DTS, as Matroska, to `path`, with the ffmpeg on
/// `PATH`, for a test to Remux.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_h264_with_5_1(path: &Path, audio: &str) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        ffmpeg
            .args(["-f", "lavfi", "-i", "testsrc=size=320x240:rate=24"])
            .args(["-f", "lavfi", "-i", "sine,aformat=channel_layouts=5.1", "-t", "14"])
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                // As most H.264 has, so a keyframe decodes before it shows.
                "-bf",
                "2",
                "-g",
                "48",
                "-sc_threshold",
                "0",
            ])
            // ffmpeg's DTS encoder is marked experimental.
            .args(["-c:a", audio, "-strict", "experimental", "-f", "matroska"])
    })
}

/// Writes 2 seconds of 1080p H.264 video with stereo AAC audio, as Matroska, to `path`, with the
/// ffmpeg on `PATH`, for a test to play within a Playback Limit.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_1080p_h264(path: &Path) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        ffmpeg
            .args(["-f", "lavfi", "-i", "testsrc=size=1920x1080:rate=24"])
            .args(["-f", "lavfi", "-i", "sine", "-t", "2"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac"])
            .args(["-f", "matroska"])
    })
}

/// Writes 20 seconds of HEVC video at 23.976 frames a second, whose frames don't fall on whole
/// seconds, with 5.1 AC-3 audio, as Matroska, to `path`, with the ffmpeg on `PATH`, for a test to
/// Transcode.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_hevc_with_5_1(path: &Path) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        ffmpeg
            .args(["-f", "lavfi", "-i", "testsrc=size=320x240:rate=24000/1001"])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine,aformat=channel_layouts=5.1",
                "-t",
                "20",
            ])
            .args(["-c:v", "libx265", "-preset", "ultrafast"])
            .args(["-x265-params", "log-level=error"])
            .args(["-c:a", "ac3", "-f", "matroska"])
    })
}

/// Writes 20 seconds of black 320×240 H.264 video with a PGS subtitle Track showing a 16×16 white
/// square at 100, 100 from its start to 19 seconds, as Matroska, to `path`, with the ffmpeg on
/// `PATH`, for a test to burn in.
///
/// # Errors
///
/// If the subtitle can't be written beside `path`, or ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_black_with_pgs(path: &Path) -> io::Result<()> {
    // ffmpeg can't encode PGS, so it muxes one written by hand.
    let sup = path.with_extension("sup");
    std::fs::write(&sup, white_square_pgs())?;
    let written = write_test_media(path, |ffmpeg| {
        ffmpeg
            .args([
                "-f",
                "lavfi",
                "-i",
                "color=black:size=320x240:rate=24",
                "-i",
            ])
            .arg(&sup)
            .args(["-t", "20", "-map", "0", "-map", "1"])
            // A keyframe each second, so a Transcode started mid-way seeks past where the
            // subtitle starts, as it would in a film.
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "24", "-c:s", "copy"])
            .args(["-f", "matroska"])
    });
    std::fs::remove_file(&sup)?;
    written
}

/// A PGS stream, as a `.sup` file holds it, on a 320×240 canvas: a 16×16 white square at 100, 100
/// shown at 0 seconds and cleared at 19.
#[cfg(any(test, feature = "testing"))]
fn white_square_pgs() -> Vec<u8> {
    // Each segment: "PG", when it shows and decodes in 90 kHz ticks, its type and its length.
    let segment = |seconds: u32, kind: u8, body: &[u8]| -> Vec<u8> {
        let length = u16::try_from(body.len()).unwrap_or(u16::MAX);
        let shows = (seconds * 90_000).to_be_bytes();
        [
            &b"PG"[..],
            &shows,
            &[0; 4],
            &[kind],
            &length.to_be_bytes(),
            body,
        ]
        .concat()
    };
    // Canvas width and height, frame rate, composition number, then its state: 0x80 starts one.
    let composition = |state: u8| vec![1, 64, 0, 240, 0x10, 0, 0, state, 0, 0];
    // Then its objects: one, in window 0 at 100, 100, or none.
    let shown = [composition(0x80), vec![1, 0, 0, 0, 0, 0, 100, 0, 100]].concat();
    let cleared = [composition(0), vec![0]].concat();
    let window = [1, 0, 0, 100, 0, 100, 0, 16, 0, 16];
    // Colour 1 is opaque white, as Y, Cr, Cb and alpha.
    let palette = [0, 0, 1, 235, 128, 128, 255];
    // Each row is 16 pixels of colour 1, then the end of the row.
    let rows = [0, 0x80 | 16, 1, 0, 0].repeat(16);
    let object = [&[0, 0, 0, 0xC0, 0, 0, 84, 0, 16, 0, 16][..], &rows].concat();
    [
        segment(0, 0x16, &shown),
        segment(0, 0x17, &window),
        segment(0, 0x14, &palette),
        segment(0, 0x15, &object),
        segment(0, 0x80, &[]),
        segment(19, 0x16, &cleared),
        segment(19, 0x17, &window),
        segment(19, 0x80, &[]),
    ]
    .concat()
}

/// Runs the ffmpeg on `PATH` with the inputs and outputs `args` adds, writing to `path`.
#[cfg(any(test, feature = "testing"))]
pub(super) fn write_test_media(
    path: &Path,
    args: impl FnOnce(&mut Command) -> &mut Command,
) -> io::Result<()> {
    let mut ffmpeg = Command::new("ffmpeg");
    let status = args(ffmpeg.args(["-v", "error", "-y"]))
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("ffmpeg failed: {status}")))
    }
}
