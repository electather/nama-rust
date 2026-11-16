use std::{fs, path::Path};

use super::{
    Ffmpeg, FromCut, Keyframe, Remux, RemuxAudio, ToneMapper, Transcode, TranscodeAudio,
    VideoConversion, VideoLimit, test_media::write_test_media, write_h264_with_5_1,
    write_hevc_with_5_1,
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

/// Remuxes the file at `path` into `dir` from `from`, its audio converted to AAC with
/// `aac_channels` if given, until ffmpeg is done.
async fn remux(
    ffmpeg: &Ffmpeg,
    path: &Path,
    dir: &Path,
    from: Keyframe,
    aac_channels: Option<i64>,
) {
    fs::create_dir_all(dir).unwrap();
    let path = path.to_str().unwrap();
    let remux = Remux {
        audio: Some(RemuxAudio {
            track: 0,
            aac_channels,
        }),
    };
    let output = ffmpeg.remux(path, &remux, dir, from).unwrap();
    let output = output.spawn().unwrap().wait_with_output().await.unwrap();
    assert!(output.status.success(), "{output:?}");
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

/// Transcodes the file at `path` into `dir`, cut every 6 seconds of its 20, from cut `n`,
/// until ffmpeg is done.
async fn transcode(ffmpeg: &Ffmpeg, path: &Path, dir: &Path, n: usize) {
    fs::create_dir_all(dir).unwrap();
    let cuts = [0, 6000, 12_000, 18_000];
    let transcode = Transcode {
        audio: Some(TranscodeAudio {
            track: 0,
            aac_channels: 2,
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

#[tokio::test]
async fn hdr10_tagged_only_by_its_transfer_is_tone_mapped_to_bt709() {
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
            limit: None,
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
        [dir.join("init-0.mp4"), dir.join("0.m4s")].map(|written| fs::read(written).unwrap());
    let played = work.path().join("played.mp4");
    fs::write(&played, init_and_first.concat()).unwrap();
    let ffprobe = &ffmpeg.ffprobe.as_ref().unwrap().path;
    let probed = tokio::process::Command::new(ffprobe)
        .args(["-v", "error", "-select_streams", "v", "-of", "csv=p=0"])
        .args(["-show_entries", "stream=color_transfer,color_primaries"])
        .arg(&played)
        .output()
        .await
        .unwrap();
    assert!(probed.status.success(), "{probed:?}");
    assert_eq!(
        String::from_utf8_lossy(&probed.stdout).trim(),
        "bt709,bt709"
    );
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
        assert!(dir.join(format!("{n}.m4s")).exists(), "{n}.m4s");
    }
    assert!(!dir.join("4.m4s").exists(), "a keyframe of its own");
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

    assert!(!restarted.join("1.m4s").exists());
    for n in 2..4 {
        let segment = |dir: &Path| fs::read(dir.join(format!("{n}.m4s"))).unwrap();
        let (expected, actual) = (
            starts_at(&segment(&whole), VIDEO),
            starts_at(&segment(&restarted), VIDEO),
        );
        assert_eq!(actual, expected, "{n}.m4s");
    }
    assert!(!restarted.join("4.m4s").exists());
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

    assert!(!restarted.join("2.m4s").exists());
    for n in 3..7 {
        let segment = |dir: &Path| fs::read(dir.join(format!("{n}.m4s"))).unwrap();
        let (expected, actual) = (
            starts_at(&segment(&whole), VIDEO),
            starts_at(&segment(&restarted), VIDEO),
        );
        // Within 2 ms, at 12288 units a second: a run's first file can round differently.
        assert!(
            expected.abs_diff(actual) <= 25,
            "{n}.m4s: {actual} against {expected}"
        );
    }
    assert!(!restarted.join("7.m4s").exists());
}

#[tokio::test]
async fn a_remux_started_at_a_keyframe_starts_its_audio_where_a_run_from_the_first_does() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("alien.mkv");
    write_h264_with_5_1(&file, "aac").unwrap();
    let first = Keyframe {
        n: 0,
        ms: 0,
        previous_ms: None,
    };
    let third = Keyframe {
        n: 3,
        ms: 6000,
        previous_ms: Some(4000),
    };
    // At 44100 units a second: copied within 2 ms, as a run's first file can round differently,
    // and converted to AAC within its frame of 1024 samples, which can fall differently.
    for (aac_channels, within) in [(None, 88), (Some(2), 1024)] {
        let run = |name: &str| work.path().join(format!("{name}-{aac_channels:?}"));
        let (whole, restarted) = (run("whole"), run("restarted"));
        remux(&ffmpeg, &file, &whole, first, aac_channels).await;

        remux(&ffmpeg, &file, &restarted, third, aac_channels).await;

        let segment = |dir: &Path| fs::read(dir.join("3.m4s")).unwrap();
        let (expected, actual) = (
            starts_at(&segment(&whole), AUDIO),
            starts_at(&segment(&restarted), AUDIO),
        );
        assert!(
            expected.abs_diff(actual) <= within,
            "{aac_channels:?}: {actual} against {expected}"
        );
    }
}
