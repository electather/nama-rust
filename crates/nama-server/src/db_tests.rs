#[cfg(feature = "testing")]
#[test]
fn advance_clock_moves_now_and_now_ms_ahead_by_what_it_adds() {
    use std::time::Duration;

    use super::{advance_clock, now, now_ms};

    const DAY_SECS: i64 = 24 * 60 * 60;
    let (secs_before, ms_before) = (now(), now_ms());

    advance_clock(Duration::from_hours(2 * 24));

    let (secs_after, ms_after) = (now(), now_ms());
    // Two days ahead, plus however long the test took, which is well under a day.
    assert!(
        (secs_before + 2 * DAY_SECS..secs_before + 3 * DAY_SECS).contains(&secs_after),
        "{secs_before} then {secs_after}"
    );
    assert!(
        (ms_before + 2 * DAY_SECS * 1000..ms_before + 3 * DAY_SECS * 1000).contains(&ms_after),
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
