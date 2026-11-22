//! The structure checks the Server's and the Hub's `tests/structure.rs` run on their own files
//! (ADR 0051, ADR 0099). Each crate passes its own tables in a [`Structure`]; the rules are under
//! "Structure" in `crates/nama-server/AGENTS.md`.

mod rows;
mod surfaces;

use std::{
    fs,
    iter::successors,
    path::{Path, PathBuf},
};

pub use rows::{Allowed, Importers};
use rows::{area_of, code_only, crate_paths, stale_allowances, upward_calls};
pub use surfaces::Exception;

/// The line count a file not in `ceilings` may reach.
const MAX_LINES: usize = 800;

/// One crate's structure tables, which [`Structure::check`] holds its files to.
#[derive(Debug)]
pub struct Structure {
    /// The crate's folder, its `env!("CARGO_MANIFEST_DIR")`.
    pub root: &'static str,
    /// Today's files over 800 lines, sorted by path, each with the ceiling it stays at or under.
    pub ceilings: &'static [(&'static str, usize)],
    /// The tables a "One owner" rule names, each with its owner's path without `.rs`.
    pub owned_tables: &'static [(&'static str, &'static [&'static str])],
    /// The crate's modules in rows of who may call whom, highest row first (ADR 0051).
    pub rows: &'static [&'static [&'static str]],
    /// Whether `rows` may name modules not written yet, as the Hub's name the areas ADR 0099
    /// plans. Otherwise a module `rows` names without a file fails.
    pub rows_name_areas_to_come: bool,
    /// The modules that start the crate, which call any row.
    pub startup: &'static [&'static str],
    /// The calls against `rows` allowed anyway. An entry no import uses fails.
    pub allowed_upward: &'static [Allowed],
    /// The modules neighbours reach inside an area anyway. An entry no import uses fails.
    pub surface_exceptions: &'static [Exception],
    /// The crates only one area's files under `src/` may name: the crate as code names it, the
    /// area, and why.
    pub confined_crates: &'static [(&'static str, &'static str, &'static str)],
    /// What the walks must find.
    pub must_find: MustFind,
}

/// What a crate's walks must find, so one that breaks fails instead of checking nothing.
#[derive(Debug)]
pub struct MustFind {
    /// A crate-relative folder two or more deep, such as `src/playback/choices`.
    pub nested_folder: &'static str,
    /// An area and one of its modules, the area holding an `AGENTS.md`; `None` while the crate
    /// has no area folders.
    pub area_module: Option<(&'static str, &'static str)>,
    /// The fewest `crate::` paths the crate's files name.
    pub crate_paths: usize,
    /// The fewest paths the crate's own `AGENTS.md` names.
    pub named_paths: usize,
}

impl Structure {
    /// Checks the crate's files against its tables.
    ///
    /// # Panics
    ///
    /// Listing every problem found, or when a walk finds less than `must_find`.
    pub fn check(&self) {
        let root = Path::new(self.root);
        let files = rust_files(root);
        let mut problems = size_problems(&files, self.ceilings);
        problems.extend(unsorted_ceilings(self.ceilings));
        problems.extend(owned_table_problems(&files, self.owned_tables));
        problems.extend(unchecked_query_problems(&files));
        problems.extend(named_path_problems(root, self.must_find.named_paths));
        problems.extend(self.row_placement_problems(&files));
        problems.extend(stray_test_problems(&files, self.must_find.nested_folder));
        problems.extend(self.upward_call_problems(&files));
        problems.extend(surfaces::deep_reach_problems(
            &files,
            self.surface_exceptions,
            &self.must_find,
        ));
        problems.extend(surfaces::long_agents_files(
            root,
            self.must_find.area_module,
        ));
        problems.extend(confined_crate_problems(&files, self.confined_crates));
        assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }

    /// What is wrong with where the `.rs` files directly under `src/` are placed in `rows` and
    /// `startup`.
    fn row_placement_problems(&self, files: &[SourceFile]) -> Vec<String> {
        let modules: Vec<&str> = files
            .iter()
            .filter_map(|file| file.path.strip_prefix("src/")?.strip_suffix(".rs"))
            .filter(|stem| !stem.contains('/') && !stem.ends_with("_tests"))
            .collect();
        assert!(
            modules.contains(&"lib"),
            "found no src/lib.rs: the walk is checking nothing"
        );
        let mut problems = placement_problems(&modules, self.rows, self.startup);
        if !self.rows_name_areas_to_come {
            problems.extend(gone_from_rows(&modules, self.rows));
        }
        problems
    }

    /// Each call against `rows` that `allowed_upward` doesn't let through, and each entry of it
    /// nothing uses.
    fn upward_call_problems(&self, files: &[SourceFile]) -> Vec<String> {
        let mut imports = 0;
        let mut problems = Vec::new();
        let mut used = vec![false; self.allowed_upward.len()];
        for file in files
            .iter()
            .filter(|file| area_of(&file.path).is_some_and(|area| !self.startup.contains(&area)))
        {
            let paths = crate_paths(&code_only(&file.text));
            imports += paths.len();
            problems.extend(upward_calls(
                &file.path,
                &paths,
                self.rows,
                self.allowed_upward,
                &mut used,
            ));
        }
        problems.extend(stale_allowances(self.allowed_upward, &used));
        assert!(
            imports >= self.must_find.crate_paths,
            "read only {imports} crate:: paths: the parser is missing them"
        );
        problems
    }
}

/// A hand-written Rust file of a crate: its crate-relative path with `/` separators, and its
/// text.
#[derive(Debug)]
pub struct SourceFile {
    /// Its crate-relative path, such as `src/lib.rs`.
    pub path: String,
    /// What it holds.
    pub text: String,
}

/// Every `.rs` file under the crate at `root`'s `src/` and `tests/`, plus `build.rs`, sorted by
/// path.
#[must_use]
pub fn rust_files(root: &Path) -> Vec<SourceFile> {
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
/// ceiling in `ceilings`.
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

/// Each file over 800 lines that `ceilings` doesn't hold, and each entry of `ceilings` that is
/// wrong.
fn size_problems(files: &[SourceFile], ceilings: &[(&str, usize)]) -> Vec<String> {
    let mut problems: Vec<String> = files
        .iter()
        .filter_map(|file| {
            let ceiling = ceilings
                .iter()
                .find(|(path, _)| *path == file.path)
                .map(|&(_, ceiling)| ceiling);
            problem(&file.path, Some(file.text.lines().count()), ceiling)
        })
        .collect();
    problems.extend(
        ceilings
            .iter()
            .filter(|(path, _)| !files.iter().any(|file| file.path == *path))
            .filter_map(|&(path, ceiling)| problem(path, None, Some(ceiling))),
    );
    problems
}

/// The first pair of `ceilings` out of order by path, or naming one path twice.
fn unsorted_ceilings(ceilings: &[(&str, usize)]) -> Option<String> {
    ceilings.windows(2).find_map(|pair| match pair {
        [(first, _), (second, _)] if first >= second => Some(format!(
            "CEILINGS lists {first} before {second}: keep it sorted by path with each path once"
        )),
        _ => None,
    })
}

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

/// Each SQL naming an owned table outside its owner's files, and each owned table its owner's
/// files never name.
fn owned_table_problems(
    files: &[SourceFile],
    owned_tables: &[(&str, &[&'static str])],
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen_in_owner: Vec<&str> = Vec::new();
    for file in files
        .iter()
        .filter(|file| file.path != "tests/structure.rs")
    {
        for (keyword, word) in sql_tables(&file.text) {
            let Some((owner, table)) = owned_tables.iter().find_map(|(owner, tables)| {
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
    for (owner, tables) in owned_tables {
        for table in tables.iter().filter(|table| !seen_in_owner.contains(table)) {
            problems.push(format!(
                "{table} is never named in SQL under {owner}: \
                 the table or its owner's path is stale"
            ));
        }
    }
    problems
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
#[must_use]
pub fn is_test_file(path: &str) -> bool {
    path.ends_with("_tests.rs") || path.ends_with("/testing.rs")
}

/// Whether `line` is a `#[cfg(test)]` or `#[cfg(all(test, ...))]` attribute, which turns its
/// item on only under test.
// ponytail: matches the trimmed line exactly, so a test-only `#[cfg(test)] // why` or
// `#[cfg( test )]` is not seen.
fn is_test_only_cfg(line: &str) -> bool {
    let line = line.trim();
    line == "#[cfg(test)]"
        || line
            .strip_prefix("#[cfg(all(")
            .and_then(|rest| rest.strip_suffix("))]"))
            .is_some_and(|terms| terms.split(',').any(|term| term.trim() == "test"))
}

/// Each `(line number, call)` in `production_lines(text)` that calls one of `UNCHECKED_QUERIES`.
// ponytail: a text scan; a call split across lines or reached through a `use` alias is not seen.
fn unchecked_queries(text: &str) -> Vec<(usize, &'static str)> {
    production_lines(text)
        .flat_map(|(number, line)| {
            UNCHECKED_QUERIES
                .iter()
                .filter(|call| line.contains(**call))
                .map(move |call| (number, *call))
        })
        .collect()
}

/// Each `(line number, line)` of `text` up to a trailing inline `mod tests` block behind a
/// test-only `#[cfg(...)]`.
pub fn production_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let tests_start = text
        .lines()
        .zip(text.lines().skip(1))
        .position(|(line, next)| is_test_only_cfg(line) && next.trim() == "mod tests {")
        .unwrap_or(usize::MAX);
    (1..).zip(text.lines().take(tests_start))
}

/// Each call of an unchecked sqlx query function in production code.
fn unchecked_query_problems(files: &[SourceFile]) -> Vec<String> {
    files
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
        .collect()
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

/// Each path an `AGENTS.md` under `root` names that exists neither beside it nor from the
/// workspace root. The crate's own `AGENTS.md` must name at least `min_named` paths.
#[expect(clippy::unwrap_used, reason = "an unreadable file fails the test")]
fn named_path_problems(root: &Path, min_named: usize) -> Vec<String> {
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
                paths.len() >= min_named,
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
    problems
}

/// What is wrong with where `modules`, the stems of the `.rs` files directly under `src/`, are
/// placed in `rows` and `startup`: a module with no place, or one placed twice.
fn placement_problems(modules: &[&str], rows: &[&[&str]], startup: &[&str]) -> Vec<String> {
    let in_rows = rows.iter().flat_map(|row| row.iter().copied());
    let placed: Vec<&str> = startup.iter().copied().chain(in_rows).collect();
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
    problems
}

/// Each module `rows` names that isn't among `modules`.
fn gone_from_rows(modules: &[&str], rows: &[&[&str]]) -> Vec<String> {
    rows.iter()
        .flat_map(|row| row.iter())
        .filter(|name| !modules.contains(name))
        .map(|name| format!("src/{name}.rs is gone: remove {name} from ROWS, or put back the file"))
        .collect()
}

/// A problem for each test file among `paths`, crate-relative, that sits at any depth under
/// `src/` beside a folder of its own in `folders`, crate-relative folder paths.
fn stray_tests(paths: &[&str], folders: &[&str]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| path.starts_with("src/"))
        .filter_map(|path| {
            let folder = path.strip_suffix("_tests.rs")?;
            let (_, name) = folder.rsplit_once('/')?;
            folders.contains(&folder).then(|| {
                format!("{path} sits beside {folder}/: move it to {folder}/{name}_tests.rs")
            })
        })
        .collect()
}

/// Each unit test file beside a folder of its own: the rule is "Unit tests sit beside their
/// file" in `crates/nama-server/AGENTS.md`. The walk must find `nested_folder`.
fn stray_test_problems(files: &[SourceFile], nested_folder: &str) -> Vec<String> {
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let mut folders: Vec<&str> = paths
        .iter()
        .flat_map(|path| successors(Some(*path), |path| Some(path.rsplit_once('/')?.0)).skip(1))
        .collect();
    folders.sort_unstable();
    folders.dedup();
    assert!(
        paths.contains(&"src/lib.rs") && folders.contains(&nested_folder),
        "found no src/lib.rs or {nested_folder}/: the walk is checking nothing"
    );
    stray_tests(&paths, &folders)
}

/// Each line of a file under `src/` outside its area that names one of `confined`, a crate only
/// that area may name.
fn confined_crate_problems(files: &[SourceFile], confined: &[(&str, &str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for file in files {
        let Some(from) = area_of(&file.path) else {
            continue;
        };
        for (line, code) in (1..).zip(code_only(&file.text).lines()) {
            let words: Vec<&str> = code
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .collect();
            for (name, area, why) in confined
                .iter()
                .filter(|(name, area, _)| from != *area && words.contains(name))
            {
                problems.push(format!(
                    "{}:{line}: names {name} outside {area} ({why})",
                    file.path
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
