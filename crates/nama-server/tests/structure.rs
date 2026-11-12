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
    ("src/playback.rs", 2000),
    ("src/playback/running.rs", 1200),
    ("src/playback/running_tests.rs", 1200),
    ("src/playback_tests.rs", 2700),
    ("src/plugins.rs", 1100),
    ("src/plugins/installed.rs", 1100),
    ("src/probing.rs", 1700),
    ("src/qualities.rs", 1200),
    ("src/requests.rs", 2700),
    ("src/requests_tests.rs", 2500),
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
    ("tests/plugins.rs", 900),
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
            "{path} is {lines} lines, over {MAX_LINES}: split it, or list it in CEILINGS if a move-only change made it out of a listed file"
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

/// The tables a "One owner" rule in `AGENTS.md` names, each with its owner's path without `.rs`.
const OWNED_TABLES: &[(&str, &[&str])] = &[
    (
        "src/contents",
        &[
            "library_files",
            "library_items",
            "item_matches",
            "match_pins",
            "title_search",
        ],
    ),
    (
        "src/tracks",
        &["version_probes", "tracks", "probe_failures"],
    ),
    ("src/qualities", &["qualities", "default_qualities"]),
    ("src/server_settings", &["server_settings"]),
    ("src/images", &["image_thumbhashes"]),
    ("src/custom_images", &["custom_images"]),
    ("src/track_preferences", &["track_preferences"]),
    ("src/watch_states", &["watch_states"]),
];

/// The keywords a table's name follows in SQL, upper case as every query here writes them.
const SQL_KEYWORDS: &[&str] = &["FROM", "JOIN", "INTO", "UPDATE", "TABLE"];

/// Whether the file at `path` is the owner's `.rs`, its `_tests.rs` or under its folder.
fn belongs_to(path: &str, owner: &str) -> bool {
    path == format!("{owner}.rs")
        || path == format!("{owner}_tests.rs")
        || path.starts_with(&format!("{owner}/"))
}

/// Each `(keyword, word)` pair in `text` where `word` directly follows one of `SQL_KEYWORDS`.
// ponytail: a word scan, not a SQL parser. Lower-case SQL, or a table reached only as
// `table.column` in another table's query, is not seen; parse the SQL if that starts to matter.
fn sql_tables(text: &str) -> Vec<(&str, &str)> {
    let words: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    words
        .iter()
        .zip(words.iter().skip(1))
        .filter(|(keyword, _)| SQL_KEYWORDS.contains(keyword))
        .map(|(keyword, word)| (*keyword, *word))
        .collect()
}

#[test]
fn an_owned_tables_sql_appears_only_in_its_owners_files() {
    let files = rust_files();
    let mut problems = Vec::new();
    let mut seen_in_owner: Vec<&str> = Vec::new();
    for file in files
        .iter()
        .filter(|file| file.path != "tests/structure.rs")
    {
        for (keyword, word) in sql_tables(&file.text) {
            let Some((owner, table)) = OWNED_TABLES.iter().find_map(|(owner, tables)| {
                tables
                    .iter()
                    .find(|table| **table == word)
                    .map(|table| (*owner, *table))
            }) else {
                continue;
            };
            if belongs_to(&file.path, owner) {
                seen_in_owner.push(table);
            } else {
                problems.push(format!("{}: {keyword} {table} (owner: {owner})", file.path));
            }
        }
    }
    for (owner, tables) in OWNED_TABLES {
        for table in tables.iter().filter(|table| !seen_in_owner.contains(table)) {
            problems.push(format!(
                "{table} is never named in SQL under {owner}: the table or its owner's path is stale"
            ));
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn sql_tables_reads_the_word_after_a_keyword_across_lines() {
    assert_eq!(
        sql_tables("SELECT x FROM\n    watch_states w JOIN tracks ON from tracks"),
        [("FROM", "watch_states"), ("JOIN", "tracks")]
    );
}

/// The paths `text`, an `AGENTS.md`, names: inline code spans outside fenced blocks that hold a `/`.
// ponytail: a bare file name without a `/` (`README.md`, `libraries.rs`) is not checked, since
// it rarely says which folder it means; name it with its folder to have it checked.
fn named_paths(text: &str) -> Vec<&str> {
    let mut fenced = false;
    let mut paths = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        // Spans sit between backticks, so they are the odd pieces of a split.
        paths.extend(line.split('`').skip(1).step_by(2).filter(|span| {
            span.contains('/')
                && !span.starts_with('/')
                && !span.contains(char::is_whitespace)
                && !span.contains(['*', '<', '>', '{', ':'])
        }));
    }
    paths
}

#[expect(clippy::unwrap_used, reason = "an unreadable folder fails the test")]
fn collect_agents_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                collect_agents_files(&path, paths);
            }
        } else if path.file_name().is_some_and(|name| name == "AGENTS.md") {
            paths.push(path);
        }
    }
}

#[test]
fn every_path_an_agents_md_names_exists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = root.parent().unwrap().parent().unwrap();
    let mut agents_files = Vec::new();
    collect_agents_files(root, &mut agents_files);
    assert!(
        agents_files.contains(&root.join("AGENTS.md")),
        "the walk found no {}: it is checking nothing",
        root.join("AGENTS.md").display()
    );
    let mut problems = Vec::new();
    for agents in &agents_files {
        let text = fs::read_to_string(agents).unwrap();
        let paths = named_paths(&text);
        let folder = agents.parent().unwrap();
        let shown = agents.strip_prefix(workspace).unwrap().display();
        if agents == &root.join("AGENTS.md") {
            assert!(
                paths.len() >= 10,
                "{shown} named only {} paths: the extractor is missing them",
                paths.len()
            );
        }
        for path in paths {
            if !folder.join(path).exists() && !workspace.join(path).exists() {
                problems.push(format!("{shown}: {path}"));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn named_paths_skips_fences_urls_globs_and_bare_names() {
    let text = "`src/a.rs` `/media/` `*.rs` `src/*.rs` `src/{a,b}.rs` `a/<id>` `x::y/z` `README.md` `a b/c`\n```\n`src/fenced.rs`\n```\n`.sqlx/`";
    assert_eq!(named_paths(text), ["src/a.rs", ".sqlx/"]);
}

#[test]
fn ceilings_are_sorted_by_path_and_list_no_path_twice() {
    if let Some(pair) = CEILINGS.windows(2).find(|pair| pair[0].0 >= pair[1].0) {
        panic!(
            "CEILINGS lists {} before {}: keep it sorted by path with each path once",
            pair[0].0, pair[1].0
        );
    }
}
