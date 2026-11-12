//! How the Server crate's files are laid out. The rules are under "Structure" in `crates/nama-server/AGENTS.md`.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The line count a file not in `CEILINGS` may reach.
const MAX_LINES: usize = 800;

/// Today's files over `MAX_LINES`, sorted by path, each with the ceiling it stays at or under.
const CEILINGS: &[(&str, usize)] = &[
    ("src/browse.rs", 1200),
    ("src/contents.rs", 2800),
    ("src/ffmpeg.rs", 2600),
    ("src/images.rs", 900),
    ("src/matching.rs", 1900),
    ("src/playback.rs", 4600),
    ("src/playback/running.rs", 2300),
    ("src/plugins.rs", 1200),
    ("src/plugins/installed.rs", 1600),
    ("src/probing.rs", 1700),
    ("src/qualities.rs", 1200),
    ("src/requests.rs", 5200),
    ("src/scanner.rs", 1500),
    ("src/server_settings.rs", 1000),
    ("src/sign_in.rs", 1100),
    ("src/tracks.rs", 900),
    ("src/users.rs", 1900),
    ("tests/access.rs", 900),
    ("tests/api.rs", 900),
    ("tests/browse.rs", 1600),
    ("tests/delete_media.rs", 1400),
    ("tests/playback.rs", 2500),
    ("tests/plugin_service.rs", 1100),
    ("tests/plugins.rs", 1100),
    ("tests/requests.rs", 4300),
    ("tests/sign_in.rs", 1400),
    ("tests/users.rs", 1200),
];

/// A hand-written Rust file of the crate: its crate-relative path with `/` separators, and its text.
struct SourceFile {
    path: String,
    text: String,
}

/// Every `.rs` file under the crate's `src/` and `tests/`, plus `build.rs`, sorted by path.
fn rust_files() -> Vec<SourceFile> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = vec![root.join("build.rs")];
    for dir in ["src", "tests"] {
        collect_rust_files(&root.join(dir), &mut paths);
    }
    let mut files: Vec<SourceFile> = paths
        .into_iter()
        .filter(|path| path.is_file())
        .map(|path| read(root, &path))
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[expect(clippy::unwrap_used, reason = "an unreadable folder fails the test")]
fn collect_rust_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

#[expect(clippy::unwrap_used, reason = "an unreadable file fails the test")]
fn read(root: &Path, path: &Path) -> SourceFile {
    let relative = path.strip_prefix(root).unwrap();
    // CI also runs on Windows, whose separator is `\`.
    let parts: Vec<_> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect();
    SourceFile {
        path: parts.join("/"),
        text: fs::read_to_string(path).unwrap(),
    }
}

/// What is wrong with one file's size, given its line count (`None` when it's gone) and its ceiling in `CEILINGS`.
fn problem(path: &str, lines: Option<usize>, ceiling: Option<usize>) -> Option<String> {
    match (lines, ceiling) {
        (Some(lines), None) if lines > MAX_LINES => Some(format!(
            "{path} is {lines} lines, over {MAX_LINES}: split it (CEILINGS takes no new files)"
        )),
        (_, None) => None,
        (None, Some(_)) => Some(format!("{path} is gone: remove its CEILINGS entry")),
        (Some(lines), Some(_)) if lines <= MAX_LINES => Some(format!(
            "{path} is {lines} lines, {MAX_LINES} or fewer: remove its CEILINGS entry"
        )),
        (Some(lines), Some(ceiling)) if lines > ceiling => Some(format!(
            "{path} is {lines} lines, over its ceiling of {ceiling}: shrink it"
        )),
        (Some(lines), Some(ceiling)) => {
            let fitting = lines.div_ceil(100) * 100;
            (ceiling != fitting).then(|| {
                format!("{path} is {lines} lines: set its ceiling from {ceiling} to {fitting}")
            })
        }
    }
}

#[test]
fn no_file_is_over_800_lines_except_those_listed_with_a_ceiling() {
    let files = rust_files();
    let mut problems: Vec<String> = files
        .iter()
        .filter_map(|file| {
            let ceiling = CEILINGS
                .iter()
                .find(|(path, _)| *path == file.path)
                .map(|&(_, ceiling)| ceiling);
            problem(&file.path, Some(file.text.lines().count()), ceiling)
        })
        .collect();
    problems.extend(
        CEILINGS
            .iter()
            .filter(|(path, _)| !files.iter().any(|file| file.path == *path))
            .filter_map(|&(path, ceiling)| problem(path, None, Some(ceiling))),
    );
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn a_ceiling_only_comes_down() {
    let fails = |lines, ceiling| problem("src/f.rs", lines, ceiling).is_some();
    assert!(!fails(Some(800), None), "unlisted at 800 lines");
    assert!(fails(Some(801), None), "unlisted over 800 lines");
    assert!(!fails(Some(1001), Some(1100)), "listed inside its band");
    assert!(!fails(Some(1100), Some(1100)), "listed at its ceiling");
    assert!(fails(Some(1101), Some(1100)), "listed over its ceiling");
    assert!(fails(Some(800), Some(900)), "listed at 800 lines or fewer");
    assert!(fails(None, Some(900)), "listed and gone");
    assert!(fails(Some(1000), Some(1100)), "ceiling could be lower");
    assert!(
        fails(Some(1001), Some(1050)),
        "ceiling not a multiple of 100"
    );
}
