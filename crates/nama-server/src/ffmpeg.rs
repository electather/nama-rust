//! Finding the ffmpeg and ffprobe the Server Remuxes, Transcodes and probes Versions with, which
//! come from the system, not the release (ADR 0026).

use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use serde::Deserialize;

use crate::{
    config::Config,
    tracks::{Probe, Track, TrackKind},
};

/// The oldest ffmpeg and ffprobe the Server uses, as major and minor version. HLS with fMP4
/// segments, the zscale and tonemap filters and the VAAPI, QSV and NVENC encoders that Remuxing
/// and Transcoding use all predate 4.4, and it's the oldest a supported LTS distribution ships
/// (Ubuntu 22.04), so nothing older gets tested.
const MINIMUM: (u32, u32) = (4, 4);

/// How long ffprobe may take over one file, such as one on a network share that stopped answering.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// Subtitle codecs, as ffprobe names them, that are bitmaps rather than text.
const IMAGE_SUBTITLES: &[&str] = &["dvb_subtitle", "dvd_subtitle", "hdmv_pgs_subtitle", "xsub"];

/// Why probing a file failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("the Server has no ffprobe")]
    Unavailable,
    #[error("ffprobe couldn't be run: {0}")]
    Run(#[from] std::io::Error),
    #[error("ffprobe took longer than {}s", PROBE_TIMEOUT.as_secs())]
    TimedOut(#[from] tokio::time::error::Elapsed),
    #[error("ffprobe failed: {0}")]
    Failed(String),
    #[error("ffprobe's output couldn't be read: {0}")]
    Output(#[from] serde_json::Error),
}

/// A program the Server found and ran.
#[derive(Debug, PartialEq, Eq)]
pub struct FoundProgram {
    /// Where it is.
    pub path: PathBuf,
    /// Its version as it reports it, such as `7.1.1`.
    pub version: String,
}

/// The ffmpeg and ffprobe the Server found at startup. Without both it only Direct Plays.
#[derive(Debug)]
pub struct Ffmpeg {
    /// ffmpeg, or why the Server can't use one, in plain words.
    pub ffmpeg: Result<FoundProgram, String>,
    /// ffprobe, or why the Server can't use one, in plain words.
    pub ffprobe: Result<FoundProgram, String>,
}

/// Neither found on `PATH`.
impl Default for Ffmpeg {
    fn default() -> Self {
        Self {
            ffmpeg: Err(not_on_path("ffmpeg")),
            ffprobe: Err(not_on_path("ffprobe")),
        }
    }
}

impl Ffmpeg {
    /// Runs ffmpeg and ffprobe at the paths `config` sets, or else the first of each in the
    /// directories of `path`, a `PATH` value, to read their versions. Blocks while they run.
    #[must_use]
    pub fn find(config: &Config, path: &OsStr) -> Self {
        Self {
            ffmpeg: find("ffmpeg", config.ffmpeg.as_deref(), path),
            ffprobe: find("ffprobe", config.ffprobe.as_deref(), path),
        }
    }

    /// Why the Server can only Direct Play, in plain words, or `None` when it can use both.
    #[must_use]
    pub fn direct_play_only_because(&self) -> Option<String> {
        let reasons: Vec<&str> = [&self.ffmpeg, &self.ffprobe]
            .into_iter()
            .filter_map(|program| program.as_ref().err())
            .map(String::as_str)
            .collect();
        (!reasons.is_empty()).then(|| reasons.join(" and "))
    }

    /// Whether the Server can probe files, which needs only ffprobe.
    pub(crate) const fn can_probe(&self) -> bool {
        self.ffprobe.is_ok()
    }

    /// The facts of the file at `path` and its video, audio and subtitle Tracks, as ffprobe reads
    /// them.
    pub(crate) async fn probe(&self, path: &str) -> Result<Probe, ProbeError> {
        let Ok(ffprobe) = &self.ffprobe else {
            return Err(ProbeError::Unavailable);
        };
        let output = tokio::process::Command::new(&ffprobe.path)
            .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
            .arg(path)
            // A probe given up on, or cancelled at shutdown, stops ffprobe too.
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(PROBE_TIMEOUT, output).await??;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProbeError::Failed(stderr.trim().to_owned()));
        }
        Ok(parse_probe(&output.stdout)?)
    }
}

/// What `ffprobe -print_format json -show_format -show_streams` printed.
#[derive(Deserialize)]
struct ProbeOutput {
    format: ProbedFormat,
    #[serde(default)]
    streams: Vec<ProbedStream>,
}

#[derive(Deserialize)]
struct ProbedFormat {
    format_name: String,
    /// In seconds, such as `"5.120000"`.
    duration: Option<String>,
    bit_rate: Option<String>,
}

#[derive(Deserialize)]
struct ProbedStream {
    codec_type: Option<String>,
    #[serde(default)]
    codec_name: String,
    width: Option<i64>,
    height: Option<i64>,
    /// Such as `"24000/1001"`, or `"0/0"` when unknown.
    avg_frame_rate: Option<String>,
    channels: Option<i64>,
    #[serde(default)]
    disposition: HashMap<String, u8>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

/// The [`Probe`] ffprobe's JSON output describes.
fn parse_probe(json: &[u8]) -> Result<Probe, serde_json::Error> {
    let output: ProbeOutput = serde_json::from_slice(json)?;
    let format = output.format;
    Ok(Probe {
        container: format.format_name,
        duration_ms: format
            .duration
            .and_then(|seconds| Duration::try_from_secs_f64(seconds.parse().ok()?).ok())
            .and_then(|duration| i64::try_from(duration.as_millis()).ok()),
        bitrate: format.bit_rate.and_then(|bitrate| bitrate.parse().ok()),
        tracks: output.streams.into_iter().filter_map(track).collect(),
    })
}

/// `stream` as a Track, or `None` for a stream that isn't one, such as cover art or a data
/// stream.
fn track(mut stream: ProbedStream) -> Option<Track> {
    let flag = |name: &str| stream.disposition.get(name).is_some_and(|&set| set == 1);
    let kind = match stream.codec_type.as_deref()? {
        // Cover art is a one-picture video stream.
        "video" if flag("attached_pic") => return None,
        "video" => TrackKind::Video {
            width: stream.width,
            height: stream.height,
            frame_rate: stream.avg_frame_rate.as_deref().and_then(frame_rate),
        },
        "audio" => TrackKind::Audio {
            channels: stream.channels,
        },
        "subtitle" => TrackKind::Subtitle {
            image: IMAGE_SUBTITLES.contains(&stream.codec_name.as_str()),
        },
        _ => return None,
    };
    let (default, forced) = (flag("default"), flag("forced"));
    Some(Track {
        codec: stream.codec_name,
        language: stream.tags.remove("language"),
        title: stream.tags.remove("title"),
        default,
        forced,
        kind,
    })
}

/// Frames per second from a fraction such as `24000/1001`, or `None` for `0/0`.
fn frame_rate(fraction: &str) -> Option<f64> {
    let (numerator, denominator) = fraction.split_once('/')?;
    let (numerator, denominator): (f64, f64) = (numerator.parse().ok()?, denominator.parse().ok()?);
    (denominator > 0.0).then(|| numerator / denominator)
}

/// `name` at `configured`, or else on `path`, if it's new enough, or why the Server can't use it.
fn find(name: &str, configured: Option<&Path>, path: &OsStr) -> Result<FoundProgram, String> {
    let found = match configured {
        Some(configured) => at_configured_path(name, configured)?,
        None => on_path(name, path).ok_or_else(|| not_on_path(name))?,
    };
    if is_too_old(&found.version) {
        let (major, minor) = MINIMUM;
        return Err(format!(
            "{name} {} at {} is older than {major}.{minor}, the oldest this Server supports",
            found.version,
            found.path.display()
        ));
    }
    Ok(found)
}

fn not_on_path(name: &str) -> String {
    format!("{name} wasn't found on PATH")
}

/// `name` at the path the config sets, or why it can't be run.
fn at_configured_path(name: &str, path: &Path) -> Result<FoundProgram, String> {
    // Relative to the working directory, like `data_dir`. Left relative, a bare `ffmpeg` would
    // be looked up on `PATH` when run.
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    if !path.is_file() {
        return Err(format!(
            "{name} wasn't found at the configured path {}",
            path.display()
        ));
    }
    let shown = path.display().to_string();
    run(name, path)
        .ok_or_else(|| format!("{name} at the configured path {shown} won't run or isn't {name}"))
}

/// The first `name` in the directories of `path` that runs and says which version it is.
fn on_path(name: &str, path: &OsStr) -> Option<FoundProgram> {
    let file = format!("{name}{}", env::consts::EXE_SUFFIX);
    env::split_paths(path)
        // A relative entry, such as an empty one, would run whatever is in the working directory.
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(&file))
        .filter(|candidate| candidate.is_file())
        .find_map(|candidate| run(name, candidate))
}

/// `name` at `path`, if it runs and says which version it is.
fn run(name: &str, path: PathBuf) -> Option<FoundProgram> {
    let output = Command::new(&path).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    // The first line reads like `ffmpeg version 7.1.1 Copyright (c) 2000-2025 the FFmpeg developers`.
    let version = String::from_utf8_lossy(&output.stdout)
        .strip_prefix(&format!("{name} version "))?
        .split_whitespace()
        .next()?
        .to_owned();
    Some(FoundProgram { path, version })
}

/// Whether `version` is a release older than [`MINIMUM`].
fn is_too_old(version: &str) -> bool {
    // A release says `7.1.1`, `n7.0` or `4.4.2-0ubuntu0.22.04.1`. A build from git says
    // `N-113021-g5b3fc5d`, with no release number, so it gets the benefit of the doubt.
    let mut numbers = version
        .trim_start_matches('n')
        .split(|c: char| !c.is_ascii_digit())
        .map(|number| number.parse::<u32>().ok());
    let Some(Some(major)) = numbers.next() else {
        return false;
    };
    let minor = numbers.next().flatten().unwrap_or(0);
    (major, minor) < MINIMUM
}

/// Writes a fifth of a second of 4K video with English 5.1 audio, as Matroska, to `path`, with the
/// ffmpeg on `PATH`, for a test to probe. Tests make their media this way, rather than commit
/// sample files, so they need ffmpeg and ffprobe.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_4k_with_5_1(path: &Path) -> std::io::Result<()> {
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(["-f", "lavfi", "-i", "color=size=3840x2160:rate=24"])
        .args(["-f", "lavfi", "-i", "anullsrc=channel_layout=5.1"])
        // Encoders every ffmpeg build has.
        .args(["-t", "0.2", "-c:v", "mpeg4", "-c:a", "ac3"])
        .args(["-metadata:s:a:0", "language=eng", "-f", "matroska"])
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("ffmpeg failed: {status}")))
    }
}

// The fakes are shell scripts.
#[cfg(all(test, unix))]
mod tests {
    use std::{
        ffi::OsStr,
        fs,
        os::unix::fs::PermissionsExt,
        path::Path,
        sync::{PoisonError, RwLock},
    };

    use super::{Ffmpeg, FoundProgram};
    use crate::config::Config;

    /// Held to write a fake, and shared to run them. A child forked while another test has a fake
    /// open for writing keeps it open until it execs, and running that fake then fails with
    /// "Text file busy".
    static WRITING_FAKES: RwLock<()> = RwLock::new(());

    /// Writes a fake `name` into `dir` that prints `version_line` as `-version` does.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn fake(dir: &Path, name: &str, version_line: &str) {
        let _writing = WRITING_FAKES
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\necho '{version_line}'\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// [`Ffmpeg::find`], while no fake is being written.
    fn find(config: &Config, path: &OsStr) -> Ffmpeg {
        let _running = WRITING_FAKES.read().unwrap_or_else(PoisonError::into_inner);
        Ffmpeg::find(config, path)
    }

    /// Looks only on `path`, with no paths configured.
    fn find_on(path: &OsStr) -> Ffmpeg {
        find(&Config::default(), path)
    }

    #[test]
    fn both_on_path_are_found_with_their_path_and_version() {
        let empty = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fake(
            bin.path(),
            "ffmpeg",
            "ffmpeg version 7.1.1 Copyright (c) 2000-2025",
        );
        fake(
            bin.path(),
            "ffprobe",
            "ffprobe version n7.0 Copyright (c) 2007-2024",
        );
        let path = std::env::join_paths([empty.path(), bin.path()]).unwrap();

        let found = find_on(&path);

        assert_eq!(
            found.ffmpeg,
            Ok(FoundProgram {
                path: bin.path().join("ffmpeg"),
                version: "7.1.1".to_owned(),
            })
        );
        assert_eq!(
            found.ffprobe,
            Ok(FoundProgram {
                path: bin.path().join("ffprobe"),
                version: "n7.0".to_owned(),
            })
        );
        assert_eq!(found.direct_play_only_because(), None);
    }

    #[test]
    fn with_neither_on_path_the_server_can_only_direct_play() {
        let empty = tempfile::tempdir().unwrap();

        let found = find_on(empty.path().as_os_str());

        assert_eq!(found.ffmpeg, Err("ffmpeg wasn't found on PATH".to_owned()));
        assert_eq!(
            found.direct_play_only_because(),
            Some("ffmpeg wasn't found on PATH and ffprobe wasn't found on PATH".to_owned())
        );
    }

    #[test]
    fn one_that_doesnt_run_is_skipped_for_a_later_one_on_path() {
        let broken = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fake(broken.path(), "ffmpeg", "something else entirely");
        fake(bin.path(), "ffmpeg", "ffmpeg version 7.1.1");
        let path = std::env::join_paths([broken.path(), bin.path()]).unwrap();

        let found = find_on(&path);

        assert_eq!(
            found.ffmpeg,
            Ok(FoundProgram {
                path: bin.path().join("ffmpeg"),
                version: "7.1.1".to_owned(),
            })
        );
    }

    #[test]
    fn a_program_that_doesnt_say_its_ffmpeg_version_isnt_found() {
        let bin = tempfile::tempdir().unwrap();
        fake(bin.path(), "ffmpeg", "something else entirely");
        fake(bin.path(), "ffprobe", "ffprobe version 7.1.1");

        let found = find_on(bin.path().as_os_str());

        assert_eq!(found.ffmpeg, Err("ffmpeg wasn't found on PATH".to_owned()));
        assert_eq!(
            found.direct_play_only_because(),
            Some("ffmpeg wasn't found on PATH".to_owned())
        );
    }

    #[test]
    fn a_configured_path_is_used_instead_of_path() {
        let on_path = tempfile::tempdir().unwrap();
        let configured = tempfile::tempdir().unwrap();
        fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
        fake(configured.path(), "my-ffmpeg", "ffmpeg version 6.1");
        let config = Config {
            ffmpeg: Some(configured.path().join("my-ffmpeg")),
            ..Config::default()
        };

        let found = find(&config, on_path.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Ok(FoundProgram {
                path: configured.path().join("my-ffmpeg"),
                version: "6.1".to_owned(),
            })
        );
    }

    #[test]
    fn a_configured_path_that_is_missing_or_wont_run_is_reported_with_that_path() {
        let configured = tempfile::tempdir().unwrap();
        fake(configured.path(), "ffprobe", "something else entirely");
        let config = Config {
            ffmpeg: Some("/opt/ffmpeg".into()),
            ffprobe: Some(configured.path().join("ffprobe")),
            ..Config::default()
        };

        let found = find(&config, configured.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Err("ffmpeg wasn't found at the configured path /opt/ffmpeg".to_owned())
        );
        assert_eq!(
            found.ffprobe,
            Err(format!(
                "ffprobe at the configured path {} won't run or isn't ffprobe",
                configured.path().join("ffprobe").display()
            ))
        );
    }

    #[test]
    fn a_relative_configured_path_is_from_the_working_directory_not_path() {
        let on_path = tempfile::tempdir().unwrap();
        fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
        let config = Config {
            ffmpeg: Some("ffmpeg".into()),
            ..Config::default()
        };

        let found = find(&config, on_path.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Err(format!(
                "ffmpeg wasn't found at the configured path {}",
                std::env::current_dir().unwrap().join("ffmpeg").display()
            ))
        );
    }

    #[test]
    fn a_version_below_the_minimum_is_too_old() {
        let bin = tempfile::tempdir().unwrap();
        fake(bin.path(), "ffmpeg", "ffmpeg version 4.3.1-static");
        fake(bin.path(), "ffprobe", "ffprobe version n4.4");

        let found = find_on(bin.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Err(format!(
                "ffmpeg 4.3.1-static at {} is older than 4.4, the oldest this Server supports",
                bin.path().join("ffmpeg").display()
            ))
        );
        assert!(found.ffprobe.is_ok(), "{:?}", found.ffprobe);
    }

    #[test]
    fn a_distribution_or_git_build_is_new_enough() {
        let bin = tempfile::tempdir().unwrap();
        fake(
            bin.path(),
            "ffmpeg",
            "ffmpeg version 4.4.2-0ubuntu0.22.04.1",
        );
        fake(bin.path(), "ffprobe", "ffprobe version N-113021-g5b3fc5d");

        let found = find_on(bin.path().as_os_str());

        assert_eq!(found.direct_play_only_because(), None);
    }
}

#[cfg(test)]
mod probe_output_tests {
    use super::parse_probe;
    use crate::tracks::{Probe, Track, TrackKind};

    /// Cut down from ffprobe 7.0's output for a Blu-ray remux with cover art.
    const BLU_RAY_REMUX: &str = r#"{
        "streams": [
            {"codec_name": "hevc", "codec_type": "video", "width": 3840, "height": 1600,
             "avg_frame_rate": "24000/1001", "disposition": {"default": 1, "forced": 0}},
            {"codec_name": "truehd", "codec_type": "audio", "channels": 8,
             "disposition": {"default": 1, "forced": 0},
             "tags": {"language": "eng", "title": "TrueHD Atmos 7.1"}},
            {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle",
             "disposition": {"default": 0, "forced": 1}, "tags": {"language": "fre"}},
            {"codec_name": "subrip", "codec_type": "subtitle",
             "disposition": {"default": 0, "forced": 0}, "tags": {"language": "eng"}},
            {"codec_name": "mjpeg", "codec_type": "video", "width": 600, "height": 900,
             "avg_frame_rate": "0/0", "disposition": {"attached_pic": 1}},
            {"codec_type": "attachment", "tags": {"filename": "font.ttf"}}
        ],
        "format": {"format_name": "matroska,webm", "duration": "7299.4565", "bit_rate": "61734512"}
    }"#;

    fn track(codec: &str, language: Option<&str>, default: bool, kind: TrackKind) -> Track {
        Track {
            codec: codec.to_owned(),
            language: language.map(str::to_owned),
            title: None,
            default,
            forced: false,
            kind,
        }
    }

    #[test]
    fn ffprobe_output_reads_as_tracks_without_cover_art_or_attachments() {
        let probe = parse_probe(BLU_RAY_REMUX.as_bytes()).unwrap();

        let Some(TrackKind::Video { frame_rate, .. }) = probe.tracks.first().map(|t| &t.kind)
        else {
            panic!("no video Track first: {probe:?}");
        };
        assert!(frame_rate.is_some_and(|fps| (fps - 23.976).abs() < 0.001));
        let ntsc = frame_rate.unwrap();
        assert_eq!(
            probe,
            Probe {
                container: "matroska,webm".to_owned(),
                duration_ms: Some(7_299_456),
                bitrate: Some(61_734_512),
                tracks: vec![
                    track(
                        "hevc",
                        None,
                        true,
                        TrackKind::Video {
                            width: Some(3840),
                            height: Some(1600),
                            frame_rate: Some(ntsc),
                        },
                    ),
                    Track {
                        title: Some("TrueHD Atmos 7.1".to_owned()),
                        ..track(
                            "truehd",
                            Some("eng"),
                            true,
                            TrackKind::Audio { channels: Some(8) },
                        )
                    },
                    Track {
                        forced: true,
                        ..track(
                            "hdmv_pgs_subtitle",
                            Some("fre"),
                            false,
                            TrackKind::Subtitle { image: true },
                        )
                    },
                    track(
                        "subrip",
                        Some("eng"),
                        false,
                        TrackKind::Subtitle { image: false },
                    ),
                ],
            }
        );
    }
}
