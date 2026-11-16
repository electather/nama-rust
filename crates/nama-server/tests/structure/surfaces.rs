//! Whether a neighbour imports only what an area's root re-exports, and whether each area's
//! `AGENTS.md` stays short.

use std::{
    fs,
    path::{MAIN_SEPARATOR, Path},
};

use super::{
    collect_agents_files,
    rows::{Import, area_of, code_only, crate_paths},
    rust_files,
};

/// A module of an area that neighbours may reach by its path anyway: the area, the module, and
/// why.
type Exception = (&'static str, &'static str, &'static str);

/// The modules neighbours reach inside an area anyway, each with why re-exporting it would be
/// wrong. An entry no import uses fails.
const SURFACE_EXCEPTIONS: &[Exception] = &[(
    "ffmpeg",
    "fake",
    "the crate's Fake programs rule sends every test's fake program through ffmpeg::fake::script",
)];

/// The most lines an area's `AGENTS.md` holds: its invariants, then its Neighbours.
const MAX_AGENTS_LINES: usize = 30;

/// Each `(area, module)` among `paths`, crate-relative `.rs` files: a file `src/B/<m>.rs` not
/// ending `_tests.rs`, or a folder `src/B/<m>/`.
fn area_modules<'a>(paths: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    let mut modules = Vec::new();
    for path in paths {
        let Some((area, rest)) = path
            .strip_prefix("src/")
            .and_then(|inside| inside.split_once('/'))
        else {
            continue;
        };
        let module = match rest.split_once('/') {
            Some((folder, _)) => folder,
            None => match rest.strip_suffix(".rs") {
                Some(stem) if !stem.ends_with("_tests") => stem,
                _ => continue,
            },
        };
        if !modules.contains(&(area, module)) {
            modules.push((area, module));
        }
    }
    modules
}

/// `code`, `code_only`'s text of `lib.rs` or `main.rs`, with `crate::` put before each path that
/// starts at one of `areas`, since the crate root names its areas directly
/// (`use library::Artwork;`). One inside a `crate::{…}` group comes out as `crate::{crate::…}`:
/// `crate_paths` reads it once as it is meant, and once under a first segment `crate`, which
/// names no area.
fn rooted(code: &str, areas: &[&str]) -> String {
    let mut out = String::with_capacity(code.len());
    let mut previous = None;
    for (at, c) in code.char_indices() {
        let rest = code.get(at..).unwrap_or("");
        let after_path_char =
            previous.is_some_and(|p: char| p.is_alphanumeric() || matches!(p, '_' | ':' | '$'));
        let starts_area_path = areas.iter().any(|area| {
            rest.strip_prefix(area)
                .is_some_and(|after| after.starts_with("::"))
        });
        if !after_path_char && starts_area_path {
            out.push_str("crate::");
        }
        out.push(c);
        previous = Some(c);
    }
    out
}

/// Each of `imports`, the `crate::` paths of the file at `path`, that names a module of another
/// area in `modules` and `exceptions` doesn't let through, as `file:line: crate::B::m::… reaches
/// into B: import what B.rs re-exports`, followed by each exception for B with its reason. Sets
/// `used[i]` when `exceptions[i]` lets an import through, for `stale_exceptions`.
fn deep_reaches(
    path: &str,
    imports: &[Import],
    modules: &[(&str, &str)],
    exceptions: &[Exception],
    used: &mut [bool],
) -> Vec<String> {
    let from = area_of(path);
    let mut problems = Vec::new();
    for import in imports {
        let [to, module, ..] = import.path.as_slice() else {
            continue;
        };
        if from == Some(to.as_str()) || !modules.contains(&(to.as_str(), module.as_str())) {
            continue;
        }
        let mut let_through = false;
        for ((area, excepted, _), entry_used) in exceptions.iter().zip(used.iter_mut()) {
            if area == to && excepted == module {
                let_through = true;
                *entry_used = true;
            }
        }
        if !let_through {
            let notes: Vec<String> = exceptions
                .iter()
                .filter(|(area, _, _)| area == to)
                .map(|(area, excepted, reason)| {
                    format!("\n    excepted only: crate::{area}::{excepted} ({reason})")
                })
                .collect();
            problems.push(format!(
                "{path}:{}: crate::{} reaches into {to}: import what {to}.rs re-exports{}",
                import.line,
                import.path.join("::"),
                notes.concat()
            ));
        }
    }
    problems
}

/// Each entry of `exceptions` that no import used, by `used` from `deep_reaches`, with its
/// reason.
fn stale_exceptions(exceptions: &[Exception], used: &[bool]) -> Vec<String> {
    exceptions
        .iter()
        .zip(used)
        .filter(|(_, used)| !**used)
        .map(|((area, module, reason), _)| {
            format!(
                "SURFACE_EXCEPTIONS lets neighbours reach crate::{area}::{module}, but nothing \
                 does: remove the entry ({reason})"
            )
        })
        .collect()
}

#[test]
fn a_neighbour_imports_only_an_areas_root() {
    let files = rust_files();
    let sources: Vec<_> = files
        .iter()
        .filter(|file| file.path.starts_with("src/"))
        .collect();
    let paths: Vec<&str> = sources.iter().map(|file| file.path.as_str()).collect();
    let modules = area_modules(&paths);
    assert!(
        modules.contains(&("library", "contents")),
        "found no src/library/contents/: the walk is checking nothing"
    );
    let mut areas: Vec<&str> = modules.iter().map(|(area, _)| *area).collect();
    areas.dedup();
    let mut imports = 0;
    let mut problems = Vec::new();
    let mut used = vec![false; SURFACE_EXCEPTIONS.len()];
    for file in sources {
        let mut code = code_only(&file.text);
        if matches!(file.path.as_str(), "src/lib.rs" | "src/main.rs") {
            code = rooted(&code, &areas);
        }
        let file_imports = crate_paths(&code);
        imports += file_imports.len();
        problems.extend(deep_reaches(
            &file.path,
            &file_imports,
            &modules,
            SURFACE_EXCEPTIONS,
            &mut used,
        ));
    }
    problems.extend(stale_exceptions(SURFACE_EXCEPTIONS, &used));
    assert!(
        imports > 100,
        "read only {imports} crate:: paths: the parser is missing them"
    );
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn area_modules_names_files_and_folders_but_not_tests_or_root_files() {
    let paths = [
        "src/lib.rs",
        "src/library.rs",
        "src/library/artwork.rs",
        "src/library/artwork_tests.rs",
        "src/library/contents.rs",
        "src/library/contents/ids.rs",
        "src/library/contents/ids_tests.rs",
        "src/scanner/scanner_tests.rs",
    ];
    assert_eq!(
        area_modules(&paths),
        [("library", "artwork"), ("library", "contents")]
    );
}

#[test]
fn rooted_prefixes_only_paths_the_crate_root_starts_at_an_area() {
    let code = "pub use library::{Artwork};\nuse crate::{library::contents::X, config::Y};\n\
                let a = crate::library::B; let b = nama_server::library::C; mod library;\n\
                let c = my_library::D; $crate::library::E; playback::run();";
    assert_eq!(
        rooted(code, &["library", "playback"]),
        "pub use crate::library::{Artwork};\nuse crate::{crate::library::contents::X, \
         config::Y};\nlet a = crate::library::B; let b = nama_server::library::C; mod library;\n\
         let c = my_library::D; $crate::library::E; crate::playback::run();"
    );
}

#[test]
fn deep_reaches_fails_a_path_into_another_areas_module_unless_excepted() {
    let modules = [("near", "inner"), ("near", "open"), ("far", "part")];
    let exceptions: &[Exception] = &[("near", "open", "why open"), ("far", "gone", "why gone")];
    let mut used = vec![false; exceptions.len()];
    let mut reaches = |path: &str, text: &str| {
        let mut code = code_only(text);
        if path == "src/lib.rs" {
            code = rooted(&code, &["near", "far"]);
        }
        deep_reaches(path, &crate_paths(&code), &modules, exceptions, &mut used)
    };
    assert_eq!(
        reaches(
            "src/far/a.rs",
            "use crate::near::{Root, open::Fake};\nlet x = crate::near::inner::Deep;"
        ),
        [
            "src/far/a.rs:2: crate::near::inner::Deep reaches into near: import what near.rs \
             re-exports\n    excepted only: crate::near::open (why open)"
        ]
    );
    let none = Vec::<String>::new();
    assert_eq!(
        reaches("src/near/b_tests.rs", "use crate::near::inner::Deep;"),
        none
    );
    assert_eq!(
        reaches("src/far.rs", "use crate::{near::Root, far::part::P};"),
        none
    );
    assert_eq!(
        reaches("src/lib.rs", "pub use far::part::P;"),
        [
            "src/lib.rs:1: crate::far::part::P reaches into far: import what far.rs re-exports\n    \
             excepted only: crate::far::gone (why gone)"
        ]
    );
    assert_eq!(reaches("src/lib.rs", "use far::Root;"), none);
    assert_eq!(
        stale_exceptions(exceptions, &used),
        [
            "SURFACE_EXCEPTIONS lets neighbours reach crate::far::gone, but nothing does: \
             remove the entry (why gone)"
        ]
    );
}

/// What is wrong with the `AGENTS.md` at `path` holding `text`, if it is over
/// `MAX_AGENTS_LINES`.
fn too_long(path: &str, text: &str) -> Option<String> {
    let lines = text.lines().count();
    (lines > MAX_AGENTS_LINES).then(|| {
        format!(
            "{path} is {lines} lines, over {MAX_AGENTS_LINES}: keep its invariants, then its \
             Neighbours"
        )
    })
}

#[test]
fn every_area_agents_md_is_30_lines_or_fewer() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut agents_files = Vec::new();
    collect_agents_files(&src, &mut agents_files);
    agents_files.retain(|path| path.parent() != Some(src.as_path()));
    assert!(
        agents_files.contains(&src.join("library").join("AGENTS.md")),
        "found no src/library/AGENTS.md: the walk is checking nothing"
    );
    let problems: Vec<String> = agents_files
        .iter()
        .filter_map(|path| {
            let shown = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .replace(MAIN_SEPARATOR, "/");
            too_long(&format!("src/{shown}"), &fs::read_to_string(path).unwrap())
        })
        .collect();
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn too_long_names_a_file_over_30_lines_and_its_length() {
    let thirty = "line\n".repeat(30);
    assert_eq!(too_long("src/a/AGENTS.md", &thirty), None);
    assert_eq!(
        too_long("src/a/AGENTS.md", &format!("{thirty}one more\n")).as_deref(),
        Some("src/a/AGENTS.md is 31 lines, over 30: keep its invariants, then its Neighbours")
    );
}
