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

/// Opens the database in `dir` as [`super::Database::open`]'s writer does, foreign keys on.
async fn connect(dir: &std::path::Path) -> sqlx::SqliteConnection {
    use sqlx::{Connection as _, sqlite::SqliteConnectOptions};

    let options = SqliteConnectOptions::new()
        .filename(dir.join(super::FILE_NAME))
        .create_if_missing(true)
        .foreign_keys(true);
    sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_upgrade_that_rebuilds_a_referenced_table_keeps_the_rows_that_reference_it() {
    static FIXTURE: sqlx::migrate::Migrator =
        sqlx::migrate!("tests/fixtures/migrations/rebuild_keeps_children");
    let dir = tempfile::tempdir().unwrap();
    let mut conn = connect(dir.path()).await;

    super::migrate(&mut conn, dir.path(), &FIXTURE)
        .await
        .unwrap();

    let children: Vec<(i64, i64)> =
        sqlx::query_as("SELECT id, parent_id FROM children ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .unwrap();
    assert_eq!(children, [(1, 1), (2, 1), (3, 2)]);
}

#[tokio::test]
async fn an_upgrade_that_fails_part_way_applies_none_of_its_migrations() {
    static FIXTURE: sqlx::migrate::Migrator =
        sqlx::migrate!("tests/fixtures/migrations/third_fails");
    let dir = tempfile::tempdir().unwrap();
    let mut conn = connect(dir.path()).await;

    let opened = super::migrate(&mut conn, dir.path(), &FIXTURE).await;

    assert!(opened.is_err(), "{opened:?}");
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_schema WHERE name IN ('firsts', 'seconds')")
            .fetch_all(&mut conn)
            .await
            .unwrap();
    assert_eq!(tables, Vec::<String>::new());
    let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_upgrade_that_leaves_a_broken_foreign_key_names_its_row_and_applies_nothing() {
    static FIXTURE: sqlx::migrate::Migrator =
        sqlx::migrate!("tests/fixtures/migrations/broken_key");
    let dir = tempfile::tempdir().unwrap();
    let mut conn = connect(dir.path()).await;

    let opened = super::migrate(&mut conn, dir.path(), &FIXTURE).await;

    let Err(super::OpenError::BrokenForeignKey {
        table,
        rowid,
        parent,
    }) = opened
    else {
        panic!("{opened:?}");
    };
    assert_eq!(
        (table.as_str(), rowid, parent.as_str()),
        ("children", Some(3), "parents")
    );
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_schema WHERE name IN ('parents', 'children')",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(tables, 0);
    let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_database_a_newer_server_upgraded_says_to_run_that_version() {
    static FIXTURE: sqlx::migrate::Migrator =
        sqlx::migrate!("tests/fixtures/migrations/rebuild_keeps_children");
    let dir = tempfile::tempdir().unwrap();
    let mut conn = connect(dir.path()).await;
    super::migrate(&mut conn, dir.path(), &FIXTURE)
        .await
        .unwrap();
    // What a newer Server's migration would have recorded.
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (99, 'newer', TRUE, x'00', 0)",
    )
    .execute(&mut conn)
    .await
    .unwrap();

    let opened = super::migrate(&mut conn, dir.path(), &FIXTURE).await;

    let Err(err @ super::OpenError::NewerVersion(99)) = opened else {
        panic!("{opened:?}");
    };
    assert!(err.to_string().contains("newer version of Nama"), "{err}");
}
