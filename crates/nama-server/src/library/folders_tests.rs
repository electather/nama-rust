use std::fs;

use tempfile::TempDir;

use super::{FolderProblem, ServerFolder, list_server_folders};

fn path_of(dir: &TempDir, name: &str) -> String {
    dir.path().join(name).to_str().unwrap().to_owned()
}

fn folder(dir: &TempDir, name: &str) -> ServerFolder {
    ServerFolder {
        name: name.to_owned(),
        path: path_of(dir, name),
    }
}

#[tokio::test]
async fn lists_only_the_folders_directly_inside_sorted_by_name_ignoring_case() {
    let dir = TempDir::new().unwrap();
    for name in ["b", "A", "c/inner", ".config"] {
        fs::create_dir_all(dir.path().join(name)).unwrap();
    }
    fs::write(dir.path().join("a.mkv"), "video").unwrap();

    let listed = list_server_folders(dir.path().to_str().unwrap())
        .await
        .unwrap();

    assert_eq!(
        listed,
        [folder(&dir, "A"), folder(&dir, "b"), folder(&dir, "c")]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_to_a_folder_is_listed_at_its_own_path_and_other_symlinks_are_not() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("media")).unwrap();
    fs::write(dir.path().join("file"), "").unwrap();
    symlink(dir.path().join("media"), dir.path().join("linked")).unwrap();
    symlink(dir.path().join("file"), dir.path().join("to-file")).unwrap();
    symlink(dir.path().join("gone"), dir.path().join("broken")).unwrap();

    let listed = list_server_folders(dir.path().to_str().unwrap())
        .await
        .unwrap();

    assert_eq!(listed, [folder(&dir, "linked"), folder(&dir, "media")]);
}

#[cfg(unix)]
#[tokio::test]
async fn children_the_server_cant_stat_are_left_out() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("locked/inner")).unwrap();
    let locked = dir.path().join("locked");
    // Read without search permission: its names list, but nothing in it can be stat'ed.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o444)).unwrap();

    let listed = list_server_folders(locked.to_str().unwrap()).await;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(listed.unwrap(), []);
}

#[tokio::test]
async fn a_missing_path_or_a_file_is_not_found() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.mkv"), "video").unwrap();

    for path in [path_of(&dir, "gone"), path_of(&dir, "a.mkv")] {
        let err = list_server_folders(&path).await.unwrap_err();
        assert!(matches!(err, FolderProblem::NotFound(_)), "{path}: {err}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_folder_without_read_permission_is_not_readable() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new().unwrap();
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let listed = list_server_folders(locked.to_str().unwrap()).await;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(matches!(listed, Err(FolderProblem::NotReadable(_))));
}

#[tokio::test]
async fn a_relative_path_is_not_absolute() {
    let err = list_server_folders("media").await.unwrap_err();

    assert!(matches!(err, FolderProblem::NotAbsolute));
}

#[cfg(unix)]
#[tokio::test]
async fn no_path_lists_the_root() {
    assert_eq!(
        list_server_folders("").await.unwrap(),
        list_server_folders("/").await.unwrap()
    );
}
