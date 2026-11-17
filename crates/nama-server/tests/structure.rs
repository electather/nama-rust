//! How the Server crate's files are laid out. The rules are under "Structure" in
//! `crates/nama-server/AGENTS.md`.

#[path = "structure/rows.rs"]
mod rows;
#[path = "structure/surfaces.rs"]
mod surfaces;

use std::{
    fs,
    path::{Path, PathBuf},
};

use rows::{Allowed, Importers, area_of, code_only, crate_paths, stale_allowances, upward_calls};

/// The line count a file not in `CEILINGS` may reach.
const MAX_LINES: usize = 800;

/// Today's files over `MAX_LINES`, sorted by path, each with the ceiling it stays at or under.
const CEILINGS: &[(&str, usize)] = &[
    ("tests/access.rs", 900),
    ("tests/api.rs", 900),
    ("tests/browse.rs", 1700),
    ("tests/delete_media.rs", 1400),
    ("tests/playback.rs", 2600),
    ("tests/plugin_service.rs", 1100),
    ("tests/plugins.rs", 900),
    ("tests/requests.rs", 4200),
    ("tests/sign_in.rs", 1400),
    ("tests/users.rs", 1200),
];

/// A hand-written Rust file of the crate: its crate-relative path with `/` separators, and its
/// text.
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

/// What is wrong with one file's size, given its line count (`None` when it's gone) and its
/// ceiling in `CEILINGS`.
fn problem(path: &str, lines: Option<usize>, ceiling: Option<usize>) -> Option<String> {
    match (lines, ceiling) {
        (Some(lines), None) if lines > MAX_LINES => Some(format!(
            "{path} is {lines} lines, over {MAX_LINES}: split it, or list it in CEILINGS if a \
             move-only change made it out of a listed file"
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
        "src/library/contents",
        &[
            "library_files",
            "library_items",
            "item_matches",
            "item_match_ids",
            "match_pins",
            "title_search",
        ],
    ),
    (
        "src/playback/tracks",
        &["version_probes", "tracks", "probe_failures"],
    ),
    (
        "src/requests/qualities",
        &["qualities", "default_qualities"],
    ),
    ("src/requests", &["request_title_ids", "followed_show_ids"]),
    ("src/server_settings", &["server_settings"]),
    ("src/images", &["image_thumbhashes"]),
    ("src/library/custom_images", &["custom_images"]),
    ("src/playback/track_preferences", &["track_preferences"]),
    ("src/playback/sidecars", &["sidecar_subtitles"]),
    ("src/library/watch_states", &["watch_states"]),
    ("src/users/personal_settings", &["personal_settings"]),
    ("src/automation/store", &["plugin_store"]),
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
                "{table} is never named in SQL under {owner}: \
                 the table or its owner's path is stale"
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

/// The calls of sqlx's query functions that `.sqlx/` doesn't check at compile time.
const UNCHECKED_QUERIES: &[&str] = &[
    "sqlx::query(",
    "sqlx::query_as(",
    "sqlx::query_scalar(",
    "sqlx::query::<",
    "sqlx::query_as::<",
    "sqlx::query_scalar::<",
    "query_with(",
    "query_as_with(",
    "query_scalar_with(",
];

/// Whether the file at `path` is test code by its name.
fn is_test_file(path: &str) -> bool {
    path.ends_with("_tests.rs") || path.ends_with("/testing.rs")
}

/// Whether `line` is a `#[cfg(test)]` or `#[cfg(all(test, ...))]` attribute, which turns its
/// item on only under test.
// ponytail: `test` nested inside an `any(...)` within the `all(...)` is not seen.
fn is_test_only_cfg(line: &str) -> bool {
    let line = line.trim();
    line == "#[cfg(test)]"
        || line
            .strip_prefix("#[cfg(all(")
            .and_then(|rest| rest.strip_suffix("))]"))
            .is_some_and(|terms| terms.split(',').any(|term| term.trim() == "test"))
}

/// Each `(line number, call)` in `text` that calls one of `UNCHECKED_QUERIES`, up to a
/// trailing inline `mod tests` block behind a test-only `#[cfg(...)]`.
// ponytail: a text scan; a call split across lines or reached through a `use` alias is not seen.
fn unchecked_queries(text: &str) -> Vec<(usize, &'static str)> {
    let tests_start = text
        .lines()
        .zip(text.lines().skip(1))
        .position(|(line, next)| is_test_only_cfg(line) && next.trim() == "mod tests {")
        .unwrap_or(usize::MAX);
    text.lines()
        .take(tests_start)
        .enumerate()
        .flat_map(|(index, line)| {
            UNCHECKED_QUERIES
                .iter()
                .filter(|call| line.contains(**call))
                .map(move |call| (index + 1, *call))
        })
        .collect()
}

#[test]
fn production_code_calls_only_sqlxs_checked_query_macros() {
    let problems: Vec<String> = rust_files()
        .iter()
        .filter(|file| file.path.starts_with("src/") && !is_test_file(&file.path))
        .flat_map(|file| {
            unchecked_queries(&file.text)
                .into_iter()
                .map(|(line, call)| {
                    format!(
                        "{}:{line}: {call} is unchecked: use a checked macro such as sqlx::query!",
                        file.path
                    )
                })
        })
        .collect();
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn unchecked_queries_finds_calls_before_an_inline_test_module() {
    let text = "let a = sqlx::query!(\"x\");\n\
                let b = sqlx::query_scalar(\"x\");\n\
                let c = sqlx::query_with(\"x\", args);\n\
                let d = sqlx::query::<Sqlite>(\"x\");\n\
                let e = sqlx::query_as::<_, Row>(\"x\");\n\
                let f = sqlx::query_scalar::<_, i64>(\"x\");\n\
                let g = sqlx::query_as_with(\"x\", args);\n\
                let h = sqlx::query_scalar_with(\"x\", args);\n\
                #[cfg(all(test, unix))]\n\
                mod tests {\n\
                    let i = sqlx::query_as(\"x\");\n\
                }";
    assert_eq!(
        unchecked_queries(text),
        [
            (2, "sqlx::query_scalar("),
            (3, "query_with("),
            (4, "sqlx::query::<"),
            (5, "sqlx::query_as::<"),
            (6, "sqlx::query_scalar::<"),
            (7, "query_as_with("),
            (8, "query_scalar_with("),
        ]
    );
    let plain_cfg = "#[cfg(test)]\nmod tests {\nlet a = sqlx::query(\"x\");\n}";
    assert_eq!(unchecked_queries(plain_cfg), []);
    assert!(is_test_file("src/a_tests.rs") && is_test_file("src/requests/testing.rs"));
    assert!(!is_test_file("src/automation/store.rs"));
}

/// The paths `text`, an `AGENTS.md`, names: inline code spans outside fenced blocks that hold a
/// `/`.
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
    let text = "`src/a.rs` `/media/` `*.rs` `src/*.rs` `src/{a,b}.rs` `a/<id>` `x::y/z` \
                `README.md` `a b/c`\n```\n`src/fenced.rs`\n```\n`.sqlx/`";
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

/// The Server's modules in rows of who may call whom, highest row first (ADR 0051). A module may
/// call a lower row, or a module to its right in its own row; `STARTUP` calls any row. The calls
/// allowed against the order are in `ALLOWED_UPWARD`.
// `forwarded` sits in the bottom row: it reads only `config`, and api and sessions use it. `web`
// imports nothing from the crate. `plugin_settings` stays a single file: its folder holds only
// private helpers (`stored.rs`, `put_back.rs`, `running.rs`) and its one rule, "One owner for
// Plugin changes", stays in the crate's AGENTS.md.
const ROWS: &[&[&str]] = &[
    &["api", "automation"],
    &["browse", "scanner", "plugin_settings"],
    &["requests", "playback", "matching"],
    &["library"],
    &["events", "connections", "users"],
    &["plugins"],
    &[
        "images",
        "server_settings",
        "ffmpeg",
        "forwarded",
        "web",
        "db",
        "page",
        "config",
        "diagnostics",
    ],
];

/// The modules that start the Server, which call any row.
const STARTUP: &[&str] = &["lib", "main"];

/// What is wrong with where `modules`, the stems of the `.rs` files directly under `src/`, are
/// placed in `rows` and `startup`.
fn placement_problems(modules: &[&str], rows: &[&[&str]], startup: &[&str]) -> Vec<String> {
    let in_rows: Vec<&str> = rows.iter().flat_map(|row| row.iter().copied()).collect();
    let placed: Vec<&str> = startup.iter().chain(&in_rows).copied().collect();
    let mut problems = Vec::new();
    for module in modules.iter().filter(|module| !placed.contains(module)) {
        problems.push(format!("src/{module}.rs has no row: add it to ROWS"));
    }
    for (index, name) in placed.iter().enumerate() {
        if placed.iter().take(index).any(|earlier| earlier == name) {
            problems.push(format!(
                "{name} is placed twice across STARTUP and ROWS: keep one"
            ));
        }
    }
    for name in in_rows.iter().filter(|name| !modules.contains(name)) {
        problems.push(format!(
            "src/{name}.rs is gone: remove {name} from ROWS, or put back the file"
        ));
    }
    problems
}

#[test]
fn every_top_level_file_has_a_row() {
    let files = rust_files();
    let modules: Vec<&str> = files
        .iter()
        .filter_map(|file| file.path.strip_prefix("src/")?.strip_suffix(".rs"))
        .filter(|stem| !stem.contains('/') && !stem.ends_with("_tests"))
        .collect();
    assert!(
        modules.contains(&"lib"),
        "found no src/lib.rs: the walk is checking nothing"
    );
    let problems = placement_problems(&modules, ROWS, STARTUP);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn placement_problems_names_unplaced_twice_placed_and_gone_modules() {
    let rows: &[&[&str]] = &[&["a"], &["b", "gone", "lib"]];
    let problems = placement_problems(&["lib", "a", "b", "loose"], rows, &["lib"]);
    assert_eq!(
        problems,
        [
            "src/loose.rs has no row: add it to ROWS",
            "lib is placed twice across STARTUP and ROWS: keep one",
            "src/gone.rs is gone: remove gone from ROWS, or put back the file",
        ]
    );
    assert_eq!(
        placement_problems(&["lib", "a"], &[&["a"]], &["lib"]),
        Vec::<String>::new()
    );
}

/// The test files among `paths`, crate-relative, that sit directly under `src/` beside a folder
/// of their own in `folders`, the names of `src/`'s folders.
fn stray_root_tests(paths: &[&str], folders: &[&str]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| {
            let stem = path.strip_prefix("src/")?.strip_suffix("_tests.rs")?;
            (!stem.contains('/') && folders.contains(&stem)).then(|| {
                format!("{path} sits beside src/{stem}/: move it to src/{stem}/{stem}_tests.rs")
            })
        })
        .collect()
}

/// The rule is "Unit tests sit beside their file" in `crates/nama-server/AGENTS.md`.
#[test]
fn an_area_roots_unit_tests_sit_in_its_folder() {
    let files = rust_files();
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let mut folders: Vec<&str> = paths
        .iter()
        .filter_map(|path| path.strip_prefix("src/")?.split_once('/'))
        .map(|(folder, _)| folder)
        .collect();
    folders.dedup();
    assert!(
        paths.contains(&"src/lib.rs") && folders.contains(&"library"),
        "found no src/lib.rs or src/library/: the walk is checking nothing"
    );
    let problems = stray_root_tests(&paths, &folders);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn stray_root_tests_names_a_root_test_file_beside_its_folder() {
    let paths = [
        "src/scanner.rs",
        "src/scanner_tests.rs",
        "src/scanner/scanner_tests.rs",
        "src/db.rs",
        "src/db_tests.rs",
    ];
    assert_eq!(
        stray_root_tests(&paths, &["scanner"]),
        ["src/scanner_tests.rs sits beside src/scanner/: move it to src/scanner/scanner_tests.rs"]
    );
}

/// The calls against `ROWS` that are allowed anyway: who makes them, the leading segments of
/// the `crate::` path they name, and why. An entry no import uses fails.
const ALLOWED_UPWARD: &[Allowed] = &[
    (
        Importers::File("src/library/contents/decisions.rs"),
        &["requests", "make_available"],
        "ADR 0012: the Library makes Requests Available inside its own write",
    ),
    (
        Importers::File("src/library/contents/decisions.rs"),
        &["requests", "make_seasons_available"],
        "ADR 0012: the Library makes Requests Available inside its own write",
    ),
];

#[test]
fn no_file_calls_up_the_rows() {
    let files = rust_files();
    let mut imports = 0;
    let mut problems = Vec::new();
    let mut used = vec![false; ALLOWED_UPWARD.len()];
    for file in files
        .iter()
        .filter(|file| area_of(&file.path).is_some_and(|area| !STARTUP.contains(&area)))
    {
        let paths = crate_paths(&code_only(&file.text));
        imports += paths.len();
        problems.extend(upward_calls(
            &file.path,
            &paths,
            ROWS,
            ALLOWED_UPWARD,
            &mut used,
        ));
    }
    problems.extend(stale_allowances(ALLOWED_UPWARD, &used));
    assert!(
        imports > 100,
        "read only {imports} crate:: paths: the parser is missing them"
    );
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
