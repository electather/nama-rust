use std::{ffi::OsStr, fs, path::Path, sync::Arc, time::Duration};

use encoding_rs::{WINDOWS_1252, WINDOWS_1256};

use super::{
    Abilities, Accel, CanDo, FRENCH_SRT, Ffmpeg, FoundProgram, Gpu, Hardware, PERSIAN_SRT,
    VideoConversion, fake, fake::script, find_gpus, video_args,
};
use crate::config::Config;

/// Writes a fake `name` into `dir` that prints `version_line` as `-version` does.
fn fake(dir: &Path, name: &str, version_line: &str) {
    script(dir, name, &format!("#!/bin/sh\necho '{version_line}'\n"));
}

/// Looks only on `path`, with no paths configured.
fn find_on(path: &OsStr) -> Ffmpeg {
    Ffmpeg::find(&Config::default(), path)
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

    let found = Ffmpeg::find(&config, on_path.path().as_os_str());

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

    let found = Ffmpeg::find(&config, configured.path().as_os_str());

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
fn a_bare_configured_name_is_found_on_path() {
    let on_path = tempfile::tempdir().unwrap();
    fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
    let config = Config {
        ffmpeg: Some("ffmpeg".into()),
        ..Config::default()
    };

    let found = Ffmpeg::find(&config, on_path.path().as_os_str());

    assert_eq!(
        found.ffmpeg,
        Ok(FoundProgram {
            path: on_path.path().join("ffmpeg"),
            version: "7.1.1".to_owned(),
        })
    );
}

#[test]
fn a_renamed_bare_name_is_found_on_path_if_it_says_its_ffmpeg() {
    let on_path = tempfile::tempdir().unwrap();
    fake(
        on_path.path(),
        "jellyfin-ffmpeg",
        "ffmpeg version 7.1.1-Jellyfin",
    );
    let config = Config {
        ffmpeg: Some("jellyfin-ffmpeg".into()),
        ..Config::default()
    };

    let found = Ffmpeg::find(&config, on_path.path().as_os_str());

    assert_eq!(
        found.ffmpeg,
        Ok(FoundProgram {
            path: on_path.path().join("jellyfin-ffmpeg"),
            version: "7.1.1-Jellyfin".to_owned(),
        })
    );
}

#[test]
fn a_renamed_bare_name_that_doesnt_say_its_ffmpeg_isnt_found() {
    let on_path = tempfile::tempdir().unwrap();
    fake(on_path.path(), "ffmpeg7", "ffprobe version 7.1.1");
    let config = Config {
        ffmpeg: Some("ffmpeg7".into()),
        ..Config::default()
    };

    let found = Ffmpeg::find(&config, on_path.path().as_os_str());

    assert_eq!(
        found.ffmpeg,
        Err("ffmpeg (configured as ffmpeg7) wasn't found on PATH".to_owned())
    );
}

#[test]
fn a_bare_name_missing_from_path_is_an_error_even_with_ffmpeg_there() {
    let on_path = tempfile::tempdir().unwrap();
    fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
    let config = Config {
        ffmpeg: Some("jellyfin-ffmpeg".into()),
        ..Config::default()
    };

    let found = Ffmpeg::find(&config, on_path.path().as_os_str());

    assert_eq!(
        found.ffmpeg,
        Err("ffmpeg (configured as jellyfin-ffmpeg) wasn't found on PATH".to_owned())
    );
}

#[test]
fn a_configured_path_with_a_folder_is_that_file_not_looked_up_on_path() {
    let on_path = tempfile::tempdir().unwrap();
    let configured = tempfile::tempdir().unwrap();
    fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
    fake(configured.path(), "ffmpeg", "ffmpeg version 6.1");
    let config = Config {
        ffmpeg: Some(configured.path().join("ffmpeg")),
        ..Config::default()
    };

    let found = Ffmpeg::find(&config, on_path.path().as_os_str());

    assert_eq!(
        found.ffmpeg,
        Ok(FoundProgram {
            path: configured.path().join("ffmpeg"),
            version: "6.1".to_owned(),
        })
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

/// What `ffmpeg` can do, as its [`Abilities`] first say.
async fn can_do(ffmpeg: Ffmpeg) -> CanDo {
    Abilities::new(Arc::new(ffmpeg)).can_do().await
}

#[tokio::test]
async fn ffmpeg_can_do_what_it_lists_and_what_passes_its_test_encode() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        "*-filters*) echo ' .SC zscale  V->V  Apply resizing, colorspace and bit depth' ;;
             *-encoders*) echo ' V....D libx264  libx264 H.264 / AVC (codec h264)'; \
                      echo ' V....D h264_vaapi  H.264/AVC (VAAPI)' ;;
             *tonemap_vaapi*) exit 1 ;;
             *h264_vaapi*) exit 0 ;;
             *) exit 1 ;;",
    );
    let with_ffprobe = Ffmpeg {
        ffprobe: Ok(FoundProgram {
            path: "/bin/true".into(),
            version: "7.1.1".to_owned(),
        }),
        ..ffmpeg
    };

    assert_eq!(
        can_do(with_ffprobe).await,
        CanDo {
            runs: true,
            zscale: true,
            libx264: true,
            hardware: vec![Hardware {
                accel: Accel::Vaapi,
                gpu: None,
                tone_maps: false,
            }],
            gpus: vec![],
        }
    );
}

#[tokio::test]
async fn ffmpeg_without_zscale_libx264_or_ffprobe_can_do_no_more_than_no_ffmpeg() {
    let bin = tempfile::tempdir().unwrap();
    // As Fedora's ffmpeg-free lists them.
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        "*-filters*) echo ' ... scale  V->V  Scale the input video size' ;;
             *-encoders*) echo ' V....D libopenh264  OpenH264 H.264 / AVC (codec h264)' ;;",
    );

    let can = can_do(ffmpeg).await;

    assert_eq!(
        can,
        CanDo {
            runs: false,
            zscale: false,
            libx264: false,
            hardware: vec![],
            gpus: vec![],
        }
    );
    assert_eq!(can_do(Ffmpeg::default()).await, can);
}

#[tokio::test]
async fn what_ffmpeg_couldnt_answer_is_asked_again() {
    let bin = tempfile::tempdir().unwrap();
    let answered = bin.path().join("answered");
    // Fails to list its filters the first time, as one that timed out on a busy host.
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        &format!(
            "*-filters*) if [ -e '{answered}' ]; then echo ' .SC zscale  V->V  zscale'; \
                         else touch '{answered}'; exit 1; fi ;;",
            answered = answered.display()
        ),
    );

    let abilities = Abilities::new(Arc::new(ffmpeg));
    let first = abilities.can_do().await;
    let then = abilities.can_do().await;

    assert!(!first.zscale);
    assert!(then.zscale);
}

#[tokio::test]
async fn hardware_ffmpeg_couldnt_list_is_tested_again() {
    let bin = tempfile::tempdir().unwrap();
    let answers = bin.path().join("answers");
    // Fails to list its encoders until `answers` exists, so the first `can_do` finds none
    // whether or not the background startup test asked first.
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        &format!(
            "*-encoders*) [ -e '{answers}' ] || exit 1; \
                          echo ' V....D h264_vaapi  H.264/AVC (VAAPI)' ;;
                 *tonemap_vaapi*) exit 1 ;;
                 *h264_vaapi*) exit 0 ;;
                 *) exit 1 ;;",
            answers = answers.display()
        ),
    );

    let abilities = Abilities::new(Arc::new(ffmpeg));
    let first = abilities.can_do().await;
    fs::write(&answers, "").unwrap();
    let then = abilities.can_do().await;

    assert_eq!(first.hardware, []);
    assert_eq!(
        then.hardware,
        [Hardware {
            accel: Accel::Vaapi,
            gpu: None,
            tone_maps: false,
        }]
    );
}

#[tokio::test]
async fn shutting_down_mid_hardware_test_stops_it() {
    let bin = tempfile::tempdir().unwrap();
    let started = bin.path().join("started");
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        &format!(
            "*-encoders*) echo ' V....D h264_vaapi  H.264/AVC (VAAPI)' ;;
                 *) touch '{}'; exec sleep 60 ;;",
            started.display()
        ),
    );
    let abilities = Abilities::new(Arc::new(ffmpeg));
    let waiting = tokio::time::Instant::now();
    while !started.exists() {
        assert!(
            waiting.elapsed() < Duration::from_secs(5),
            "no test encode started"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    tokio::time::timeout(Duration::from_secs(5), abilities.shutdown())
        .await
        .unwrap();
}

/// Writes a fake ffmpeg into `dir` that runs `cases`, a shell `case` over its arguments, and
/// returns it as the Server's ffmpeg.
pub(crate) fn fake_ffmpeg(dir: &Path, cases: &str) -> Ffmpeg {
    Ffmpeg {
        ffmpeg: Ok(script(
            dir,
            "ffmpeg",
            &format!("#!/bin/sh\ncase \"$*\" in\n{cases}\nesac\n"),
        )),
        ..Ffmpeg::default()
    }
}

#[tokio::test]
async fn only_hardware_ffmpeg_lists_and_that_passes_its_test_encode_is_found() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        // NVENC is listed but fails, as without NVIDIA's driver; QSV isn't listed at all.
        "*-encoders*) echo ' V....D h264_vaapi  H.264/AVC (VAAPI)'; \
                      echo ' V....D h264_nvenc  NVIDIA NVENC H.264 encoder' ;;
             *tonemap_vaapi*) exit 1 ;;
             *h264_vaapi*) exit 0 ;;
             *) exit 1 ;;",
    );

    let found = ffmpeg.find_hardware().await;

    assert_eq!(
        found,
        Ok(vec![Hardware {
            accel: Accel::Vaapi,
            gpu: None,
            tone_maps: false,
        }])
    );
}

#[tokio::test]
async fn hardware_that_passes_its_tone_map_test_tone_maps() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(
        bin.path(),
        "*-encoders*) echo ' V..... h264_qsv  H.264 (Intel Quick Sync Video acceleration)' ;;
             *h264_qsv*) exit 0 ;;
             *) exit 1 ;;",
    );

    let found = ffmpeg.find_hardware().await;

    assert_eq!(
        found,
        Ok(vec![Hardware {
            accel: Accel::Qsv,
            gpu: None,
            tone_maps: true,
        }])
    );
}

/// Where a Transcode on `hardware` opens its GPU: what follows each `-init_hw_device`.
fn opened(hardware: &Hardware) -> Vec<String> {
    let video = VideoConversion {
        hardware: Some(hardware.clone()),
        tone_map: None,
        limit: None,
        burn_in: None,
        in_band_headers: true,
    };
    let input = video_args("alien.mkv", &video, false).input;
    (input.windows(2))
        .filter_map(|pair| match pair {
            [option, value] if option == "-init_hw_device" => Some(value.clone()),
            _ => None,
        })
        .collect()
}

/// `ffmpeg` with the GPUs in `root`'s fake `/sys` and `/dev/dri`.
fn with_gpus(ffmpeg: Ffmpeg, root: &Path) -> Ffmpeg {
    Ffmpeg {
        gpus: find_gpus(&root.join("dri"), &root.join("sys/class/drm")),
        ..ffmpeg
    }
}

#[tokio::test]
async fn automatic_picks_the_dedicated_gpu_whichever_render_node_it_has() {
    for (dedicated, integrated) in [("renderD128", "renderD129"), ("renderD129", "renderD128")] {
        let root = tempfile::tempdir().unwrap();
        let dri = root.path().join("dri");
        fake::two_amd_gpus_at(root.path(), [integrated, dedicated]);
        let ffmpeg = fake_ffmpeg(
            root.path(),
            "*-encoders*) echo ' V....D h264_vaapi  H.264/AVC (VAAPI)' ;;
                 *tonemap_vaapi*) exit 1 ;;
                 *h264_vaapi*) exit 0 ;;
                 *) exit 1 ;;",
        );

        let can = can_do(with_gpus(ffmpeg, root.path())).await;

        let ids: Vec<_> = (can.hardware.iter())
            .map(|hardware| hardware.gpu.as_ref().map(Gpu::id))
            .collect();
        assert_eq!(ids, [Some("0000:03:00.0"), Some("0000:18:00.0")]);
        let node = dri.join(dedicated).display().to_string();
        assert_eq!(
            opened(can.automatic().unwrap()),
            [format!("vaapi=nama:{node}")]
        );
    }
}

#[tokio::test]
async fn an_intel_gpu_is_tested_with_qsv_and_vaapi_and_an_amd_one_with_vaapi_alone() {
    let root = tempfile::tempdir().unwrap();
    let (sys, dri) = (root.path().join("sys"), root.path().join("dri"));
    fake::gpu(&sys, &dri, "renderD128", "0000:00:02.0", "0x8086");
    fake::gpu(&sys, &dri, "renderD129", "0000:03:00.0", "0x1002");
    let runs = root.path().join("runs");
    // VAAPI on the Intel GPU fails its test encode, and on the AMD one its tone-map.
    let body = format!(
        "#!/bin/sh
echo \"$*\" >> '{runs}'
case \"$*\" in
             *-v\\ verbose*renderD129*) echo 'VAAPI driver: Mesa Gallium driver 26.2.6 for AMD Radeon \
                                RX 9070 XT (radeonsi, gfx1201)' >&2 ;;
             *-encoders*) echo ' V....D h264_vaapi  H.264/AVC (VAAPI)'; \
                      echo ' V..... h264_qsv  H.264 (Intel Quick Sync Video acceleration)' ;;
             *vaapi=nama:*renderD128*) exit 1 ;;
             *tonemap_vaapi*) exit 1 ;;
             *) exit 0 ;;
             esac\n",
        runs = runs.display()
    );
    let ffmpeg = Ffmpeg {
        ffmpeg: Ok(script(root.path(), "ffmpeg", &body)),
        ..Ffmpeg::default()
    };

    let found = with_gpus(ffmpeg, root.path())
        .find_hardware()
        .await
        .unwrap();

    let found: Vec<_> = (found.iter())
        .map(|hardware| (hardware.name(), hardware.tone_maps, opened(hardware)))
        .collect();
    let node = |node: &str| dri.join(node).display().to_string();
    assert_eq!(
        found,
        [
            (
                "VAAPI on the AMD Radeon RX 9070 XT".to_owned(),
                false,
                vec![format!("vaapi=nama:{}", node("renderD129"))]
            ),
            (
                "Intel QSV on the Intel integrated GPU".to_owned(),
                true,
                vec![
                    format!("vaapi=va:{}", node("renderD128")),
                    "qsv=nama@va".to_owned()
                ]
            ),
        ]
    );
    let runs = fs::read_to_string(&runs).unwrap();
    let tested = |pattern: &str| runs.lines().any(|run| run.contains(pattern));
    assert!(
        tested(&format!("vaapi=nama:{} ", node("renderD128"))),
        "{runs}"
    );
    assert!(
        !tested(&format!("vaapi=va:{} ", node("renderD129"))),
        "{runs}"
    );
}

#[tokio::test]
async fn without_ffmpeg_no_hardware_is_found() {
    assert_eq!(Ffmpeg::default().find_hardware().await, Ok(vec![]));
}

/// What a fake ffmpeg that prints its arguments is given to read the sidecar `bytes` as WebVTT.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sidecar_read_with(bytes: &[u8]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), r#"*) echo "$@" ;;"#);
    let sidecar = dir.path().join("Alien.srt");
    fs::write(&sidecar, bytes).unwrap();
    let sidecar = sidecar.to_str().unwrap();

    let read = ffmpeg.subtitle(sidecar, None, "webvtt", "webvtt").await;

    String::from_utf8(read.unwrap()).unwrap()
}

#[tokio::test]
async fn a_sidecar_not_in_utf8_is_read_in_the_charset_its_bytes_are_in() {
    let persian = sidecar_read_with(&WINDOWS_1256.encode(PERSIAN_SRT).0).await;
    let french = sidecar_read_with(&WINDOWS_1252.encode(FRENCH_SRT).0).await;

    assert!(
        persian.starts_with("-v error -nostdin -sub_charenc windows-1256 -i "),
        "{persian}"
    );
    assert!(
        french.starts_with("-v error -nostdin -sub_charenc windows-1252 -i "),
        "{french}"
    );
}

#[tokio::test]
async fn a_utf8_sidecar_with_or_without_a_bom_is_read_with_no_charset() {
    let plain = sidecar_read_with(FRENCH_SRT.as_bytes()).await;
    let with_bom = sidecar_read_with(format!("\u{feff}{FRENCH_SRT}").as_bytes()).await;

    assert!(!plain.contains("-sub_charenc"), "{plain}");
    assert!(!with_bom.contains("-sub_charenc"), "{with_bom}");
}
