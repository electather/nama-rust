use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{
    AudioConversion, AudioEncoder, Ffmpeg, FromCut, Keyframe, Remux, RemuxAudio, ToneMapper,
    Transcode, TranscodeAudio, VideoConversion, VideoLimit, test_media::write_test_media,
    write_h264_with_5_1, write_hevc_with_5_1,
};
use crate::config::Config;

/// The ffmpeg and ffprobe on `PATH`, which these tests need.
fn system_ffmpeg() -> Ffmpeg {
    let ffmpeg = Ffmpeg::find(&Config::default(), &std::env::var_os("PATH").unwrap());
    assert!(
        ffmpeg.can_remux(),
        "this test needs ffmpeg and ffprobe on PATH"
    );
    ffmpeg
}

/// Remuxes the file at `path`, which starts at `file_start_ms`, into `dir` from `from`, as
/// `remux` says, until ffmpeg is done.
#[expect(
    clippy::too_many_arguments,
    reason = "forwards `Ffmpeg::remux`'s arguments, and takes the ffmpeg to run them on"
)]
async fn remux_with(
    ffmpeg: &Ffmpeg,
    path: &Path,
    remux: &Remux,
    dir: &Path,
    from: Keyframe,
    file_start_ms: i64,
) {
    fs::create_dir_all(dir).unwrap();
    let path = path.to_str().unwrap();
    let output = ffmpeg.remux(path, remux, dir, from, file_start_ms).unwrap();
    let output = output.spawn().unwrap().wait_with_output().await.unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Remuxes the file at `path` into `dir` from `from`, its audio converted as `conversion` says
/// if given, until ffmpeg is done.
async fn remux(
    ffmpeg: &Ffmpeg,
    path: &Path,
    dir: &Path,
    from: Keyframe,
    conversion: Option<AudioConversion>,
) {
    let audio = RemuxAudio {
        track: 0,
        conversion,
    };
    let remux = Remux {
        audio: Some(audio),
        hevc: false,
    };
    remux_with(ffmpeg, path, &remux, dir, from, 0).await;
}

/// The fMP4 track ffmpeg writes the video to, mapped first.
const VIDEO: u32 = 1;

/// The fMP4 track ffmpeg writes the audio to, mapped after the video.
const AUDIO: u32 = 2;

/// When the first fragment of `track` in an fMP4 segment starts, in the track's own units.
fn starts_at(segment: &[u8], track: u32) -> u64 {
    let traf = (boxes(segment, b"moof").flat_map(|moof| boxes(moof, b"traf")))
        .find(|traf| {
            let tfhd = boxes(traf, b"tfhd").next().unwrap();
            u32::from_be_bytes(tfhd[4..8].try_into().unwrap()) == track
        })
        .unwrap();
    let tfdt = boxes(traf, b"tfdt").next().unwrap();
    let (version, time) = (tfdt[0], &tfdt[4..]);
    if version == 1 {
        u64::from_be_bytes(time[..8].try_into().unwrap())
    } else {
        u32::from_be_bytes(time[..4].try_into().unwrap()).into()
    }
}

/// The contents of each MP4 box named `name` directly in `data`.
fn boxes<'a>(data: &'a [u8], name: &'a [u8; 4]) -> impl Iterator<Item = &'a [u8]> {
    let mut rest = data;
    std::iter::from_fn(move || {
        while rest.len() >= 8 {
            let size = u32::from_be_bytes(rest[..4].try_into().unwrap());
            let (this, after) = rest.split_at(usize::try_from(size).unwrap());
            rest = after;
            if &this[4..8] == name {
                return Some(&this[8..]);
            }
        }
        None
    })
}

/// The file in `dir` that a run started at keyframe `from` cut at keyframe `n`.
fn cut(dir: &Path, from: usize, n: usize) -> PathBuf {
    dir.join(format!("ffmpeg-{from}-{}.m4s", n - from))
}

/// Asserts that a Remux of a 14-second file into `restarted` from keyframe 3 of its 2-second
/// ones cut only at keyframes 3 to 6, each file's video starting within `within` of the track's
/// units of where it does in the Remux from the first into `whole`.
fn assert_same_video_from_3(whole: &Path, restarted: &Path, within: u64) {
    for n in 3..7 {
        let (expected, actual) = (
            starts_at(&fs::read(cut(whole, 0, n)).unwrap(), VIDEO),
            starts_at(&fs::read(cut(restarted, 3, n)).unwrap(), VIDEO),
        );
        assert!(
            expected.abs_diff(actual) <= within,
            "keyframe {n}: {actual} against {expected}"
        );
    }
    assert!(!cut(restarted, 3, 7).exists());
}

/// What Probing reads of the file at `path`: when it starts, in milliseconds, and when each
/// keyframe of its first video falls, in milliseconds from that start.
// Not every 2 seconds from 0: before ffmpeg 8, ffprobe reads the test media as starting at its
// AAC's priming, before 0, and ffmpeg 4.4 writes its video an AAC frame late.
fn probe(ffmpeg: &Ffmpeg, path: &Path) -> (i64, Vec<i64>) {
    let ffprobe = &ffmpeg.ffprobe.as_ref().unwrap().path;
    let output = std::process::Command::new(ffprobe)
        .args(["-v", "error", "-select_streams", "V:0", "-show_entries"])
        .args(["packet=pts_time,flags:format=start_time", "-of", "csv=p=0"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let csv = String::from_utf8(output.stdout).unwrap();
    // ffprobe prints seconds with six decimals, such as `-0.023000`.
    let ms = |seconds: &str| seconds.replace('.', "").parse::<i64>().unwrap() / 1000;
    let start = ms(csv.lines().find(|line| !line.contains(',')).unwrap());
    let mut keyframes: Vec<i64> = csv
        .lines()
        .filter_map(|line| line.split_once(','))
        .filter(|(_, flags)| flags.contains('K'))
        .map(|(seconds, _)| ms(seconds) - start)
        .collect();
    // Packets come in decoding order.
    keyframes.sort_unstable();
    (start, keyframes)
}

/// Transcodes the file at `path` into `dir`, cut every 6 seconds of its 20, from cut `n`,
/// until ffmpeg is done.
async fn transcode(ffmpeg: &Ffmpeg, path: &Path, dir: &Path, n: usize) {
    fs::create_dir_all(dir).unwrap();
    let cuts = [0, 6000, 12_000, 18_000];
    let transcode = Transcode {
        audio: Some(TranscodeAudio {
            track: 0,
            conversion: AudioConversion {
                encoder: AudioEncoder::Aac,
                channels: 2,
            },
        }),
        video: VideoConversion {
            hardware: None,
            tone_map: None,
            limit: None,
            burn_in: None,
            in_band_headers: false,
        },
    };
    let from = FromCut {
        n,
        cuts: &cuts[n..],
    };
    let output = ffmpeg
        .transcode(path.to_str().unwrap(), &transcode, dir, from)
        .unwrap();
    let output = output.spawn().unwrap().wait_with_output().await.unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// What ffprobe reads of the video a software Transcode, tone-mapping and limited to `limit`, plays
/// from a 320x240 HDR10 file tagged only by its transfer: its width, height, transfer and
/// primaries.
async fn tone_mapped_hdr10_probed(limit: Option<VideoLimit>) -> String {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("hdr10.mkv");
    write_test_media(&file, |command| {
        command
            .args([
                "-f",
                "lavfi",
                "-i",
                // The frames carry the transfer, since ffmpeg 9 drops `-color_trc`.
                "testsrc=size=320x240:rate=24,setparams=color_trc=smpte2084",
                "-t",
                "1",
            ])
            .args([
                "-c:v",
                "libx265",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p10le",
            ])
            .args(["-x265-params", "log-level=error"])
            .args(["-f", "matroska"])
    })
    .unwrap();
    let dir = work.path().join("transcoded");
    fs::create_dir_all(&dir).unwrap();
    let tone_mapped = Transcode {
        audio: None,
        video: VideoConversion {
            hardware: None,
            tone_map: Some(ToneMapper::Software),
            limit,
            burn_in: None,
            in_band_headers: false,
        },
    };
    let from = FromCut { n: 0, cuts: &[0] };

    let output = ffmpeg
        .transcode(file.to_str().unwrap(), &tone_mapped, &dir, from)
        .unwrap();

    let output = output.spawn().unwrap().wait_with_output().await.unwrap();
    assert!(output.status.success(), "{output:?}");
    let init_and_first =
        [dir.join("init-0.mp4"), cut(&dir, 0, 0)].map(|written| fs::read(written).unwrap());
    let played = work.path().join("played.mp4");
    fs::write(&played, init_and_first.concat()).unwrap();
    let ffprobe = &ffmpeg.ffprobe.as_ref().unwrap().path;
    let probed = tokio::process::Command::new(ffprobe)
        .args(["-v", "error", "-select_streams", "v", "-of", "csv=p=0"])
        .args([
            "-show_entries",
            "stream=width,height,color_transfer,color_primaries",
        ])
        .arg(&played)
        .output()
        .await
        .unwrap();
    assert!(probed.status.success(), "{probed:?}");
    String::from_utf8_lossy(&probed.stdout).trim().to_owned()
}

#[tokio::test]
async fn hdr10_tagged_only_by_its_transfer_is_tone_mapped_to_bt709() {
    assert_eq!(tone_mapped_hdr10_probed(None).await, "320,240,bt709,bt709");
}

#[tokio::test]
async fn hdr10_limited_and_tone_mapped_is_scaled_down_to_bt709() {
    let limit = Some(VideoLimit {
        size: Some((160, 120)),
        bits_per_second: 1_000_000,
    });

    assert_eq!(tone_mapped_hdr10_probed(limit).await, "160,120,bt709,bt709");
}

/// Run by hand where there is a GPU: `cargo test -p nama-server --lib -- --ignored hardware`.
#[tokio::test]
#[ignore = "needs a GPU that ffmpeg can use"]
async fn hardware_found_on_this_host_transcodes_cut_at_every_cut_and_tone_maps_hdr10() {
    let ffmpeg = system_ffmpeg();
    let found = ffmpeg.find_hardware().await.unwrap();
    let hardware = found.first().expect("no hardware passed its test encode");
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("hdr10.mkv");
    write_test_media(&file, |command| {
        command
            .args(["-f", "lavfi", "-i", "testsrc2=size=3840x2160:rate=24"])
            .args(["-t", "20", "-c:v", "libx265", "-preset", "ultrafast"])
            .args(["-pix_fmt", "yuv420p10le", "-x265-params"])
            .arg("log-level=error:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc")
            .args(["-f", "matroska"])
    })
    .unwrap();
    let tone_map = if hardware.tone_maps {
        ToneMapper::Hardware
    } else {
        ToneMapper::Software
    };
    let video = VideoConversion {
        hardware: Some(hardware.clone()),
        tone_map: Some(tone_map),
        limit: Some(VideoLimit {
            size: Some((1920, 1080)),
            bits_per_second: 8_000_000,
        }),
        burn_in: None,
        in_band_headers: true,
    };
    let dir = work.path().join("transcoded");
    fs::create_dir_all(&dir).unwrap();

    let transcode = Transcode { audio: None, video };
    let from = FromCut {
        n: 0,
        cuts: &[0, 6000, 12_000, 18_000],
    };

    let output = ffmpeg
        .transcode(file.to_str().unwrap(), &transcode, &dir, from)
        .unwrap();

    let output = output.spawn().unwrap().wait_with_output().await.unwrap();
    assert!(output.status.success(), "{hardware:?}: {output:?}");
    for n in 0..4 {
        assert!(cut(&dir, 0, n).exists(), "cut {n}");
    }
    assert!(!cut(&dir, 0, 4).exists(), "a keyframe of its own");
}

#[tokio::test]
async fn a_transcode_started_at_a_cut_writes_the_same_files_from_there_on() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("alien.mkv");
    write_hevc_with_5_1(&file).unwrap();
    let (whole, restarted) = (work.path().join("whole"), work.path().join("restarted"));
    transcode(&ffmpeg, &file, &whole, 0).await;

    transcode(&ffmpeg, &file, &restarted, 2).await;

    for n in 2..4 {
        let (expected, actual) = (
            starts_at(&fs::read(cut(&whole, 0, n)).unwrap(), VIDEO),
            starts_at(&fs::read(cut(&restarted, 2, n)).unwrap(), VIDEO),
        );
        assert_eq!(actual, expected, "cut {n}");
    }
    assert!(!cut(&restarted, 2, 4).exists());
}

#[tokio::test]
async fn a_remux_started_at_a_keyframe_writes_the_same_files_from_there_on() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("alien.mkv");
    write_h264_with_5_1(&file, "aac").unwrap();
    // A keyframe every 2 seconds of 14, each starting a file. Only Probing lists them.
    let keyframes = [0, 2000, 4000, 6000];
    let (whole, restarted) = (work.path().join("whole"), work.path().join("restarted"));
    let first = Keyframe {
        n: 0,
        ms: keyframes[0],
        previous_ms: None,
    };
    remux(&ffmpeg, &file, &whole, first, None).await;

    let third = Keyframe {
        n: 3,
        ms: keyframes[3],
        previous_ms: Some(keyframes[2]),
    };
    remux(&ffmpeg, &file, &restarted, third, None).await;

    // Within 2 ms, at 12288 units a second: a run's first file can round differently.
    assert_same_video_from_3(&whole, &restarted, 25);
}

#[tokio::test]
async fn an_hevc_remux_writes_its_video_as_hvc1_not_hev1() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("alien.mkv");
    write_hevc_with_5_1(&file).unwrap();
    let dir = work.path().join("remux");
    let first = Keyframe {
        n: 0,
        ms: 0,
        previous_ms: None,
    };

    let hevc = Remux {
        audio: None,
        hevc: true,
    };
    remux_with(&ffmpeg, &file, &hevc, &dir, first, 0).await;

    let init = fs::read(dir.join("init-0.mp4")).unwrap();
    let has = |name: &[u8]| init.windows(4).any(|window| window == name);
    assert!(has(b"hvc1"));
    assert!(!has(b"hev1"));
}

#[tokio::test]
async fn a_remux_converting_surround_to_e_ac_3_writes_e_ac_3_5_1_hls() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("alien.mkv");
    write_h264_with_5_1(&file, "dts").unwrap();
    let dir = work.path().join("remux");
    let first = Keyframe {
        n: 0,
        ms: 0,
        previous_ms: None,
    };
    let surround = AudioConversion {
        encoder: AudioEncoder::Eac3,
        channels: 6,
    };

    remux(&ffmpeg, &file, &dir, first, Some(surround)).await;

    let ffprobe = &ffmpeg.ffprobe.as_ref().unwrap().path;
    let probed = tokio::process::Command::new(ffprobe)
        .args(["-v", "error", "-select_streams", "a", "-show_entries"])
        .args(["stream=codec_name,channels", "-of", "default=nw=1:nk=1"])
        .arg(dir.join("init-0.mp4"))
        .output()
        .await
        .unwrap();
    assert!(probed.status.success(), "{probed:?}");
    assert_eq!(String::from_utf8_lossy(&probed.stdout).trim(), "eac3\n6");
}

#[tokio::test]
async fn a_remux_started_at_a_keyframe_starts_its_audio_where_a_run_from_the_first_does() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("alien.mkv");
    write_h264_with_5_1(&file, "aac").unwrap();
    let (start_ms, keyframes) = probe(&ffmpeg, &file);
    let first = Keyframe {
        n: 0,
        ms: keyframes[0],
        previous_ms: None,
    };
    let third = Keyframe {
        n: 3,
        ms: keyframes[3],
        previous_ms: Some(keyframes[2]),
    };
    // At 44100 units a second: copied within 2 ms, as a run's first file can round differently,
    // or a frame of 1024 samples from a file starting before 0 (`start_audio_at`), and converted
    // to AAC within its frame, which can fall differently.
    let copied_within = if start_ms < 0 { 1024 } else { 88 };
    let stereo_aac = AudioConversion {
        encoder: AudioEncoder::Aac,
        channels: 2,
    };
    for (conversion, within) in [(None, copied_within), (Some(stereo_aac), 1024)] {
        let run = |name: &str| work.path().join(format!("{name}-{conversion:?}"));
        let (whole, restarted) = (run("whole"), run("restarted"));
        remux(&ffmpeg, &file, &whole, first, conversion).await;

        remux(&ffmpeg, &file, &restarted, third, conversion).await;

        let (expected, actual) = (
            starts_at(&fs::read(cut(&whole, 0, 3)).unwrap(), AUDIO),
            starts_at(&fs::read(cut(&restarted, 3, 3)).unwrap(), AUDIO),
        );
        assert!(
            expected.abs_diff(actual) <= within,
            "{conversion:?}: {actual} against {expected}"
        );
    }
}

#[tokio::test]
async fn a_remux_of_a_file_starting_after_0_starts_its_copied_audio_where_a_run_from_the_first_does()
 {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let (mkv, ts) = (work.path().join("alien.mkv"), work.path().join("alien.ts"));
    write_h264_with_5_1(&mkv, "ac3").unwrap();
    // MPEG-TS starts its times late, here at about 1.5 seconds.
    write_test_media(&ts, |copying| {
        copying
            .arg("-i")
            .arg(&mkv)
            .args(["-map", "0", "-c", "copy", "-f", "mpegts"])
    })
    .unwrap();
    let (start_ms, keyframes) = probe(&ffmpeg, &ts);
    assert!(start_ms > 0, "{start_ms}");
    let copied = Remux {
        audio: Some(RemuxAudio {
            track: 0,
            conversion: None,
        }),
        hevc: false,
    };
    let first = Keyframe {
        n: 0,
        ms: keyframes[0],
        previous_ms: None,
    };
    let third = Keyframe {
        n: 3,
        ms: keyframes[3],
        previous_ms: Some(keyframes[2]),
    };
    let (whole, restarted) = (work.path().join("whole"), work.path().join("restarted"));
    remux_with(&ffmpeg, &ts, &copied, &whole, first, start_ms).await;

    remux_with(&ffmpeg, &ts, &copied, &restarted, third, start_ms).await;

    // Within 2 ms, as a run's first file can round differently: MPEG-TS video is timed at 90000
    // units a second, and this audio at 44100.
    assert_same_video_from_3(&whole, &restarted, 180);
    let (expected, actual) = (
        starts_at(&fs::read(cut(&whole, 0, 3)).unwrap(), AUDIO),
        starts_at(&fs::read(cut(&restarted, 3, 3)).unwrap(), AUDIO),
    );
    assert!(
        expected.abs_diff(actual) <= 88,
        "{actual} against {expected}"
    );
}
