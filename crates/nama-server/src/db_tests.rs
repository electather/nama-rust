#[cfg(feature = "testing")]
#[test]
fn advance_clock_moves_now_and_now_ms_ahead_by_what_it_adds() {
    use std::time::Duration;

    use super::{advance_clock, now, now_ms};

    const STEP_SECS: i64 = 2;
    // Other lib tests step the same process-wide offset by a second or two meanwhile.
    const MARGIN_SECS: i64 = 10 * 60;
    let (secs_before, ms_before) = (now(), now_ms());

    advance_clock(Duration::from_secs(STEP_SECS.unsigned_abs()));

    let (secs_after, ms_after) = (now(), now_ms());
    assert!(
        (secs_before + STEP_SECS..=secs_before + MARGIN_SECS).contains(&secs_after),
        "{secs_before} then {secs_after}"
    );
    assert!(
        (ms_before + STEP_SECS * 1000..=ms_before + MARGIN_SECS * 1000).contains(&ms_after),
        "{ms_before} then {ms_after}"
    );
}

#[test]
fn a_network_share_is_a_network_risk() {
    use super::{FilesystemRisk, filesystem_risk};

    for filesystem in [
        "nfs", "cifs", "smb2", "smbfs", "9p", "ceph", "afs", "afpfs", "webdav",
    ] {
        assert_eq!(
            filesystem_risk(filesystem),
            Some(FilesystemRisk::Network),
            "{filesystem}"
        );
    }
}

#[test]
fn zfs_and_btrfs_are_copy_on_write_risks() {
    use super::{FilesystemRisk, filesystem_risk};

    for filesystem in ["zfs", "btrfs"] {
        assert_eq!(
            filesystem_risk(filesystem),
            Some(FilesystemRisk::CopyOnWrite),
            "{filesystem}"
        );
    }
}

#[test]
fn ext4_xfs_and_apfs_are_no_risk() {
    use super::filesystem_risk;

    for filesystem in ["ext4", "xfs", "apfs"] {
        assert_eq!(filesystem_risk(filesystem), None, "{filesystem}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_names_the_magic_numbers_of_risky_filesystems_only() {
    use super::linux_filesystem_name;

    for (magic, name) in [
        (0x6969, Some("nfs")),
        (0xFF53_4D42, Some("cifs")),
        (0xFE53_4D42, Some("smb2")),
        (0x0102_1997, Some("9p")),
        (0x00C3_6400, Some("ceph")),
        (0x5346_414F, Some("afs")),
        (0x2FC1_2FC1, Some("zfs")),
        (0x9123_683E, Some("btrfs")),
        (0xEF53, None),      // ext4
        (0x5846_5342, None), // XFS
    ] {
        assert_eq!(linux_filesystem_name(magic), name, "{magic:#x}");
    }
}

#[test]
fn a_data_directory_under_one_gib_free_warns_naming_its_free_space_threshold_and_path() {
    use std::path::Path;

    use super::{MIB, warn_if_low_on_space};
    use crate::log_capture::CapturedLog;

    let log = CapturedLog::start();
    warn_if_low_on_space(Path::new("/srv/nama"), Ok(512 * MIB), 100 * MIB);

    let logged = log.finish();
    assert!(logged.contains("WARN"), "{logged}");
    for named in ["/srv/nama", "512 MiB free", "1024 MiB"] {
        assert!(logged.contains(named), "{named} in {logged}");
    }
}

#[test]
fn a_database_over_half_a_gib_asks_for_twice_its_size_free() {
    use std::path::Path;

    use super::{MIB, warn_if_low_on_space};
    use crate::log_capture::CapturedLog;

    let log = CapturedLog::start();
    warn_if_low_on_space(Path::new("/srv/nama"), Ok(1200 * MIB), 800 * MIB);

    let logged = log.finish();
    assert!(logged.contains("1200 MiB free"), "{logged}");
    assert!(logged.contains("1600 MiB"), "{logged}");
}

#[test]
fn enough_free_space_logs_nothing() {
    use std::path::Path;

    use super::{MIB, warn_if_low_on_space};
    use crate::log_capture::CapturedLog;

    let log = CapturedLog::start();
    warn_if_low_on_space(Path::new("/srv/nama"), Ok(1024 * MIB), 100 * MIB);
    warn_if_low_on_space(Path::new("/srv/nama"), Ok(2000 * MIB), 1000 * MIB);

    assert_eq!(log.finish(), "");
}

#[test]
fn free_space_that_cant_be_read_is_logged_at_debug_only() {
    use std::{io, path::Path};

    use tracing::Level;

    use super::warn_if_low_on_space;
    use crate::log_capture::CapturedLog;

    let log = CapturedLog::start_at(Level::DEBUG);
    warn_if_low_on_space(
        Path::new("/srv/nama"),
        Err(io::ErrorKind::Unsupported.into()),
        0,
    );

    let logged = log.finish();
    let lines: Vec<_> = logged.lines().collect();
    assert_eq!(lines.len(), 1, "{logged}");
    assert!(lines[0].contains("DEBUG"), "{logged}");
    assert!(lines[0].contains("free space"), "{logged}");
}
