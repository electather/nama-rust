use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
};

use nama_plugin_api::PluginId;

use super::{
    ChangeError, HardwareChoice, Invalid, PickedGpu, PlaybackChange, PlaybackLimit, PlaybackValues,
    Settings, Values,
};
#[cfg(unix)]
use crate::ffmpeg::fake::vaapi_on_two_gpus;
use crate::{db::Database, diagnostics::Sharing, ffmpeg::Accel};

const DEFAULT_FOLDER: &str = "/data/transcodes";

async fn fresh() -> (tempfile::TempDir, Settings) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    let settings = Settings::new(Arc::new(db), DEFAULT_FOLDER.into());
    settings.load().await.unwrap();
    (data_dir, settings)
}

/// A segment folder inside `data_dir`, which saving it creates.
fn folder_in(data_dir: &tempfile::TempDir) -> String {
    data_dir.path().join("segments").display().to_string()
}

/// A second `Settings` on the same database, as after a restart, once it loaded.
async fn restarted(settings: &Settings) -> Settings {
    let restarted = Settings::new(Arc::clone(&settings.db), DEFAULT_FOLDER.into());
    restarted.load().await.unwrap();
    restarted
}

fn change(folder: &str) -> PlaybackChange<'_> {
    PlaybackChange {
        segment_folder: folder,
        max_transcodes: None,
        keep_behind_minutes: None,
        hardware: HardwareChoice::Automatic,
        remote_limit: None,
    }
}

fn invalid(result: Result<impl std::fmt::Debug, ChangeError>) -> Invalid {
    match result {
        Err(ChangeError::Invalid(invalid)) => invalid,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn every_setting_is_its_default_until_one_is_stored() {
    let (_data_dir, settings) = fresh().await;
    let playback = settings.playback();

    assert_eq!(settings.now(), Values::default());
    assert_eq!(settings.server_url().request_link(7), None);
    assert_eq!(playback.now().hardware, HardwareChoice::Automatic);
    assert_eq!(playback.path(), Path::new(DEFAULT_FOLDER));
}

#[tokio::test]
async fn a_stored_url_is_trimmed_used_for_links_and_read_again_after_a_restart() {
    let (_data_dir, settings) = fresh().await;

    let stored = settings
        .change_server(" https://example.com/nama/ ", Sharing::default())
        .await
        .unwrap();

    assert_eq!(stored, "https://example.com/nama/");
    assert_eq!(
        settings.server_url().request_link(7).as_deref(),
        Some("https://example.com/nama/requests/7")
    );
    assert_eq!(restarted(&settings).await.now().url, stored);
}

#[tokio::test]
async fn sharing_starts_off_and_what_is_shared_is_read_again_after_a_restart() {
    let (_data_dir, settings) = fresh().await;
    assert_eq!(settings.now().sharing, Sharing::default());
    let sharing = Sharing {
        diagnostics: false,
        usage_analytics: true,
    };

    settings.change_server("", sharing).await.unwrap();

    assert_eq!(restarted(&settings).await.now().sharing, sharing);
}

#[tokio::test]
async fn refusing_a_url_with_a_user_name_password_query_or_fragment_or_not_http_changes_nothing() {
    let (_data_dir, settings) = fresh().await;
    settings
        .change_server("https://nama.example.com", Sharing::default())
        .await
        .unwrap();

    for not_a_url in [
        "nama.example.com",
        "ftp://nama.example.com",
        "https://nama.example.com/?a=1",
        "https://nama.example.com/#top",
        "https://user:pw@nama.example.com",
        "https://user@nama.example.com",
        "https://:pw@nama.example.com",
    ] {
        assert_eq!(
            invalid(settings.change_server(not_a_url, Sharing::default()).await),
            Invalid::NotAUrl
        );
    }
    assert_eq!(settings.now().url, "https://nama.example.com");
}

#[tokio::test]
async fn a_url_with_an_at_sign_only_in_its_path_is_accepted() {
    let (_data_dir, settings) = fresh().await;

    let stored = settings
        .change_server("https://nama.example.com/@home", Sharing::default())
        .await
        .unwrap();

    assert_eq!(stored, "https://nama.example.com/@home");
}

#[tokio::test]
async fn clearing_the_url_leaves_links_out() {
    let (_data_dir, settings) = fresh().await;
    settings
        .change_server("https://nama.example.com", Sharing::default())
        .await
        .unwrap();

    settings
        .change_server("  ", Sharing::default())
        .await
        .unwrap();

    assert_eq!(settings.server_url().request_link(7), None);
    assert_eq!(restarted(&settings).await.now().url, "");
}

#[tokio::test]
async fn stored_playback_settings_are_in_use_and_read_again_after_a_restart() {
    let (data_dir, settings) = fresh().await;
    let folder = folder_in(&data_dir);
    let playback = settings.playback();

    let stored = settings
        .change_playback(
            PlaybackChange {
                segment_folder: &format!(" {folder} "),
                max_transcodes: Some(5),
                keep_behind_minutes: Some(12),
                hardware: HardwareChoice::Api(Accel::Vaapi, None),
                remote_limit: Some(PlaybackLimit::P720),
            },
            &[],
        )
        .await
        .unwrap();

    let expected = PlaybackValues {
        segment_folder: folder.clone(),
        max_transcodes: NonZeroU32::new(5),
        keep_behind_minutes: NonZeroU32::new(12),
        hardware: HardwareChoice::Api(Accel::Vaapi, None),
        remote_limit: Some(PlaybackLimit::P720),
    };
    assert_eq!(stored, expected);
    assert_eq!(playback.now(), expected);
    assert_eq!(playback.path(), Path::new(&folder));
    assert_eq!(playback.keep_behind_ms(), 720_000);
    assert_eq!(
        playback.leftover_paths(),
        [PathBuf::from(&folder), DEFAULT_FOLDER.into()]
    );
    assert_eq!(restarted(&settings).await.playback().now(), expected);
}

#[tokio::test]
async fn playback_settings_go_back_to_their_defaults_when_cleared() {
    let (data_dir, settings) = fresh().await;
    let folder = folder_in(&data_dir);
    settings
        .change_playback(
            PlaybackChange {
                segment_folder: &folder,
                max_transcodes: Some(5),
                keep_behind_minutes: Some(12),
                hardware: HardwareChoice::Off,
                remote_limit: Some(PlaybackLimit::P720),
            },
            &[],
        )
        .await
        .unwrap();

    settings.change_playback(change(""), &[]).await.unwrap();

    let restarted = restarted(&settings).await.playback();
    assert_eq!(restarted.now(), PlaybackValues::default());
    assert_eq!(restarted.path(), Path::new(DEFAULT_FOLDER));
    assert_eq!(restarted.keep_behind_ms(), 300_000);
}

#[tokio::test]
async fn a_folder_that_isnt_absolute_is_refused_naming_it_trimmed() {
    let (_data_dir, settings) = fresh().await;

    assert_eq!(
        invalid(settings.change_playback(change(" tmp/nama "), &[]).await),
        Invalid::FolderNotAbsolute {
            folder: "tmp/nama".to_owned()
        }
    );
}

#[tokio::test]
async fn a_cap_of_0_is_refused_and_leaves_every_playback_setting_as_it_was() {
    let (data_dir, settings) = fresh().await;
    let folder = folder_in(&data_dir);

    let refused = settings
        .change_playback(
            PlaybackChange {
                max_transcodes: Some(0),
                hardware: HardwareChoice::Off,
                ..change(&folder)
            },
            &[],
        )
        .await;

    assert_eq!(invalid(refused), Invalid::NoTranscodes);
    assert_eq!(settings.playback().now(), PlaybackValues::default());
    assert_eq!(
        restarted(&settings).await.playback().now(),
        PlaybackValues::default()
    );
}

#[tokio::test]
async fn keeping_0_minutes_behind_is_refused_and_leaves_every_playback_setting_as_it_was() {
    let (data_dir, settings) = fresh().await;
    let folder = folder_in(&data_dir);

    let refused = settings
        .change_playback(
            PlaybackChange {
                keep_behind_minutes: Some(0),
                ..change(&folder)
            },
            &[],
        )
        .await;

    assert_eq!(invalid(refused), Invalid::NothingKeptBehind);
    assert_eq!(
        restarted(&settings).await.playback().now(),
        PlaybackValues::default()
    );
}

#[tokio::test]
async fn a_folder_that_isnt_absolute_is_reported_before_a_cap_of_0() {
    let (_data_dir, settings) = fresh().await;

    let refused = settings
        .change_playback(
            PlaybackChange {
                max_transcodes: Some(0),
                ..change("tmp")
            },
            &[],
        )
        .await;

    assert_eq!(
        invalid(refused),
        Invalid::FolderNotAbsolute {
            folder: "tmp".to_owned()
        }
    );
}

#[tokio::test]
async fn a_folder_saved_is_created_and_left_empty() {
    let (data_dir, settings) = fresh().await;
    let folder = data_dir.path().join("segments").join("nama");

    let change = change(folder.to_str().unwrap());
    settings.change_playback(change, &[]).await.unwrap();

    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
}

#[tokio::test]
async fn an_empty_folder_takes_the_default_without_creating_it() {
    let (data_dir, settings) = fresh().await;
    let default = data_dir.path().join("transcodes");
    let settings = Settings::new(Arc::clone(&settings.db), default.clone());

    settings.change_playback(change(""), &[]).await.unwrap();

    assert_eq!(settings.playback().path(), default);
    assert!(!default.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_folder_the_server_cant_create_or_write_is_refused_naming_it_and_nothing_changes() {
    use std::os::unix::fs::PermissionsExt;

    let (_data_dir, settings) = fresh().await;
    let locked = tempfile::tempdir().unwrap();
    std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
    if tempfile::tempfile_in(locked.path()).is_ok() {
        // Running as root, which writes anywhere; there's nothing to test.
        return;
    }

    for folder in [locked.path().to_owned(), locked.path().join("nama")] {
        let folder = folder.to_str().unwrap().to_owned();
        let refused = settings.change_playback(change(&folder), &[]).await;

        assert_eq!(invalid(refused), Invalid::FolderNotWritable { folder });
    }
    assert_eq!(std::fs::read_dir(locked.path()).unwrap().count(), 0);
    assert_eq!(
        restarted(&settings).await.playback().now(),
        PlaybackValues::default()
    );
}

fn picked(accel: Accel, id: &str, name: &str) -> HardwareChoice {
    let gpu = PickedGpu {
        id: id.to_owned(),
        name: name.to_owned(),
    };
    HardwareChoice::Api(accel, Some(gpu))
}

#[cfg(unix)]
#[tokio::test]
async fn a_picked_gpu_is_stored_under_the_name_it_was_found_with_and_read_after_a_restart() {
    let (data_dir, settings) = fresh().await;
    let passed = vaapi_on_two_gpus(data_dir.path());

    let stored = settings
        .change_playback(
            PlaybackChange {
                hardware: picked(Accel::Vaapi, "0000:18:00.0", "a name the client made up"),
                ..change("")
            },
            &passed,
        )
        .await
        .unwrap();

    let integrated = picked(Accel::Vaapi, "0000:18:00.0", "AMD integrated GPU");
    assert_eq!(stored.hardware, integrated);
    assert_eq!(settings.playback().now().hardware, integrated);
    assert_eq!(
        restarted(&settings).await.playback().now().hardware,
        integrated
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_gpu_its_api_didnt_pass_on_or_one_with_an_api_that_takes_none_is_refused() {
    let (data_dir, settings) = fresh().await;
    let passed = vaapi_on_two_gpus(data_dir.path());

    for (accel, id) in [
        (Accel::Vaapi, "0000:0a:00.0"),
        (Accel::Qsv, "0000:03:00.0"),
        (Accel::Nvenc, "0000:03:00.0"),
        (Accel::VideoToolbox, "0000:03:00.0"),
    ] {
        let refused = settings
            .change_playback(
                PlaybackChange {
                    hardware: picked(accel, id, "AMD Radeon RX 9070 XT"),
                    ..change("")
                },
                &passed,
            )
            .await;

        assert_eq!(
            invalid(refused),
            Invalid::GpuNotOffered { id: id.to_owned() },
            "{accel:?}"
        );
    }
    assert_eq!(settings.playback().now(), PlaybackValues::default());
}

#[cfg(unix)]
#[tokio::test]
async fn after_a_restart_without_its_gpu_a_pick_keeps_its_name_and_survives_another_save() {
    let (data_dir, settings) = fresh().await;
    let pick = |name| PlaybackChange {
        hardware: picked(Accel::Vaapi, "0000:03:00.0", name),
        ..change("")
    };
    let passed = vaapi_on_two_gpus(data_dir.path());
    settings.change_playback(pick(""), &passed).await.unwrap();
    let restarted = restarted(&settings).await;

    let saved_again = restarted
        .change_playback(
            PlaybackChange {
                max_transcodes: Some(3),
                ..pick("another name")
            },
            &[],
        )
        .await
        .unwrap();

    let stored = picked(Accel::Vaapi, "0000:03:00.0", "AMD Radeon RX 9070 XT");
    assert_eq!(saved_again.hardware, stored);
    assert_eq!(saved_again.max_transcodes, NonZeroU32::new(3));
}

#[tokio::test]
async fn changing_one_group_keeps_the_others_stored_values() {
    let (data_dir, settings) = fresh().await;
    let folder = folder_in(&data_dir);
    settings
        .change_server("https://nama.example.com", Sharing::default())
        .await
        .unwrap();
    settings
        .change_playback(change(&folder), &[])
        .await
        .unwrap();
    let after_playback = restarted(&settings).await.now();
    settings
        .change_server("https://other.example.com", Sharing::default())
        .await
        .unwrap();
    let after_url = restarted(&settings).await.now();

    assert_eq!(after_playback.url, "https://nama.example.com");
    assert_eq!(after_url.playback.segment_folder, folder);
}

#[tokio::test]
async fn stored_image_settings_are_in_use_and_read_again_after_a_restart() {
    let (_data_dir, settings) = fresh().await;
    let default = settings.images().resizing();

    settings.change_images(false, Some(64)).await.unwrap();
    let off = settings.images().resizing();
    settings.change_images(true, Some(64)).await.unwrap();

    assert_eq!(default, Some(1024 * 1024 * 1024));
    assert_eq!(off, None);
    assert_eq!(
        restarted(&settings).await.images().resizing(),
        Some(64 * 1024 * 1024)
    );
}

#[tokio::test]
async fn changing_the_playback_settings_first_stores_no_url() {
    let (data_dir, settings) = fresh().await;
    let folder = folder_in(&data_dir);

    settings
        .change_playback(change(&folder), &[])
        .await
        .unwrap();

    assert_eq!(restarted(&settings).await.now().url, "");
}

#[tokio::test]
async fn a_stored_image_store_is_in_use_kept_by_other_changes_and_read_after_a_restart() {
    let (_data_dir, settings) = fresh().await;
    let s3 = PluginId::from_static("s3");

    settings.set_image_store(Some(s3.clone())).await.unwrap();
    let in_use = settings.now().image_store;
    settings.change_images(false, None).await.unwrap();
    let after_images = restarted(&settings).await.now().image_store;
    settings.set_image_store(None).await.unwrap();

    assert_eq!(in_use, Some(s3.clone()));
    assert_eq!(after_images, Some(s3));
    assert_eq!(restarted(&settings).await.now().image_store, None);
}
