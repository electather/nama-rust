//! Whether a neighbour imports only what an area's root re-exports, and whether each area's
//! `AGENTS.md` stays short.

use std::{
    fs,
    path::{MAIN_SEPARATOR, Path},
};

use super::{
    MustFind, SourceFile, collect_agents_files,
    rows::{Import, area_of, code_only, crate_paths},
};

/// A module of an area that neighbours may reach by its path anyway: the area, the module, and
/// why.
pub type Exception = (&'static str, &'static str, &'static str);

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

/// `code`, `code_only`'s text, with `crate::` put before each path that starts at one of
/// `areas`, so `crate_paths` reads paths a file names without `crate::`: the crate root's to its
/// areas (`use library::Artwork;`), or an area root's to its own modules. One inside a `crate::{…}` group comes out as `crate::{crate::…}`:
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

/// Each `(area, m)` among `modules` that the area's root re-exports an item of under `m`'s own
/// name (`pub use m::{Error, m};`), reading the root's `code_only` text `code`.
fn namesake_reexports<'a>(
    area: &'a str,
    code: &str,
    modules: &[(&'a str, &'a str)],
) -> Vec<(&'a str, &'a str)> {
    let own: Vec<&str> = modules
        .iter()
        .filter(|(owner, _)| *owner == area)
        .map(|(_, module)| *module)
        .collect();
    let use_trees: String = code
        .split_inclusive(';')
        .filter_map(public_use_tree)
        .collect();
    let reexports = crate_paths(&rooted(&use_trees, &own));
    own.into_iter()
        .filter(|module| {
            reexports
                .iter()
                .any(|import| import.path == [*module, *module])
        })
        .map(|module| (area, module))
        .collect()
}

/// The tree of `statement` if it is a `use` with a `pub` visibility, such as `pub(crate)`.
// ponytail: `pub(in path) use` isn't read, so its re-export still fails as a reach; nothing
// writes one today.
fn public_use_tree(statement: &str) -> Option<&str> {
    let (before, tree) = statement.split_once(" use ")?;
    let visibility = before
        .trim_end()
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or(before);
    visibility.starts_with("pub").then_some(tree)
}

/// Each of `imports`, the `crate::` paths of the file at `path`, that names a module of another
/// area in `modules` and `exceptions` doesn't let through, as `file:line: crate::B::m::… reaches
/// into B: import what B.rs re-exports`, followed by each exception for B with its reason. Sets
/// `used[i]` when `exceptions[i]` lets an import through, for `stale_exceptions`. A re-export
/// under the module's name is no reach: `deep_reach_problems` drops `crate::B::m` first when
/// B.rs re-exports `m::m`, since that path names the item, not the module.
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

/// Each path from outside an area into one of its modules that `exceptions` doesn't let
/// through, and each entry of `exceptions` nothing uses. A path `crate::B::m` naming an item
/// B.rs re-exports under `m`'s name is no reach (`namesake_reexports`).
pub(super) fn deep_reach_problems(
    files: &[SourceFile],
    exceptions: &[Exception],
    must_find: &MustFind,
) -> Vec<String> {
    let sources: Vec<_> = files
        .iter()
        .filter(|file| file.path.starts_with("src/"))
        .collect();
    let paths: Vec<&str> = sources.iter().map(|file| file.path.as_str()).collect();
    let modules = area_modules(&paths);
    if let Some((area, module)) = must_find.area_module {
        assert!(
            modules.contains(&(area, module)),
            "found no src/{area}/{module}/: the walk is checking nothing"
        );
    }
    let mut areas: Vec<&str> = modules.iter().map(|(area, _)| *area).collect();
    areas.dedup();
    let namesakes: Vec<_> = sources
        .iter()
        .filter_map(|file| {
            let area = file.path.strip_prefix("src/")?.strip_suffix(".rs")?;
            Some(namesake_reexports(area, &code_only(&file.text), &modules))
        })
        .flatten()
        .collect();
    let mut imports = 0;
    let mut problems = Vec::new();
    let mut used = vec![false; exceptions.len()];
    for file in sources {
        let mut code = code_only(&file.text);
        if matches!(file.path.as_str(), "src/lib.rs" | "src/main.rs") {
            code = rooted(&code, &areas);
        }
        let mut file_imports = crate_paths(&code);
        imports += file_imports.len();
        file_imports.retain(|import| match import.path.as_slice() {
            [to, module] => !namesakes.contains(&(to.as_str(), module.as_str())),
            _ => true,
        });
        problems.extend(deep_reaches(
            &file.path,
            &file_imports,
            &modules,
            exceptions,
            &mut used,
        ));
    }
    problems.extend(stale_exceptions(exceptions, &used));
    assert!(
        imports >= must_find.crate_paths,
        "read only {imports} crate:: paths: the parser is missing them"
    );
    problems
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

/// Each area's `AGENTS.md` under the crate at `root` that is over `MAX_AGENTS_LINES`. The walk
/// must find the one of `area_module`'s area.
#[expect(clippy::unwrap_used, reason = "an unreadable file fails the test")]
pub(super) fn long_agents_files(root: &Path, area_module: Option<(&str, &str)>) -> Vec<String> {
    let src = root.join("src");
    let mut agents_files = Vec::new();
    collect_agents_files(&src, &mut agents_files);
    agents_files.retain(|path| path.parent() != Some(src.as_path()));
    if let Some((area, _)) = area_module {
        assert!(
            agents_files.contains(&src.join(area).join("AGENTS.md")),
            "found no src/{area}/AGENTS.md: the walk is checking nothing"
        );
    }
    agents_files
        .iter()
        .filter_map(|path| {
            let shown = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .replace(MAIN_SEPARATOR, "/");
            too_long(&format!("src/{shown}"), &fs::read_to_string(path).unwrap())
        })
        .collect()
}

#[cfg(test)]
#[path = "surfaces_tests.rs"]
mod tests;
