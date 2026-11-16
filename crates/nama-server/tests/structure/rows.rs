//! The `crate::` paths each Server file names, and which of them call up `ROWS`.

use super::is_test_file;

/// Which files an `ALLOWED_UPWARD` entry covers.
pub(super) enum Importers {
    /// The file at this crate-relative path.
    File(&'static str),
    /// Test code, the files `is_test_file` names.
    TestCode,
}

/// `text` with its comments and its string and char literals blanked to spaces, keeping every
/// line break, so a doc link or a path in a string isn't read as an import.
pub(super) fn code_only(text: &str) -> String {
    let mut code = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        let skipped = comment_or_literal_len(rest);
        let len = if skipped == 0 {
            first.len_utf8()
        } else {
            skipped
        };
        let (taken, after) = rest.split_at_checked(len).unwrap_or((rest, ""));
        if skipped == 0 {
            code.push_str(taken);
        } else {
            code.extend(taken.chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
        }
        rest = after;
    }
    code
}

/// The length in bytes of the comment or string or char literal `rest` starts with, or 0 when it
/// starts with other code, such as a lifetime.
fn comment_or_literal_len(rest: &str) -> usize {
    let mut chars = rest.chars();
    let (first, second, third) = (chars.next(), chars.next(), chars.next());
    let raw_hashes = rest
        .get(1..)
        .map(|after_r| after_r.len() - after_r.trim_start_matches('#').len());
    match (first, second, third) {
        (Some('/'), Some('/'), _) => rest.find('\n').unwrap_or(rest.len()),
        (Some('/'), Some('*'), _) => block_comment_len(rest),
        (Some('"'), _, _) => quoted_len(rest, 1, 0),
        (Some('r'), _, _)
            if raw_hashes.is_some_and(|hashes| {
                rest.get(1 + hashes..)
                    .is_some_and(|tail| tail.starts_with('"'))
            }) =>
        {
            let hashes = raw_hashes.unwrap_or(0);
            quoted_len(rest, hashes + 2, hashes)
        }
        // An escaped char, such as '\'' or '\u{e9}'.
        (Some('\''), Some('\\'), _) => rest
            .get(3..)
            .and_then(|tail| tail.find('\''))
            .map_or(rest.len(), |close| close + 4),
        (Some('\''), Some(c), Some('\'')) => c.len_utf8() + 2,
        _ => 0,
    }
}

/// The length in bytes of the block comment `rest` starts with; block comments nest.
fn block_comment_len(rest: &str) -> usize {
    let (mut depth, mut at) = (0, 0);
    while let Some(tail) = rest.get(at..).filter(|tail| !tail.is_empty()) {
        if tail.starts_with("/*") {
            depth += 1;
            at += 2;
        } else if tail.starts_with("*/") {
            depth -= 1;
            at += 2;
            if depth == 0 {
                return at;
            }
        } else {
            at += tail.chars().next().map_or(1, char::len_utf8);
        }
    }
    rest.len()
}

/// The length in bytes of the string `rest` starts with, whose text starts `open` bytes in and
/// ends at a `"` and `hashes` `#`s. Escapes count only in a plain string, the one with no `#`s.
fn quoted_len(rest: &str, open: usize, hashes: usize) -> usize {
    let close = format!("\"{}", "#".repeat(hashes));
    let mut escaped = false;
    for (at, c) in rest.char_indices().skip_while(|&(at, _)| at < open) {
        if escaped {
            escaped = false;
        } else if c == '\\' && hashes == 0 {
            escaped = true;
        } else if rest.get(at..).is_some_and(|tail| tail.starts_with(&close)) {
            return at + close.len();
        }
    }
    rest.len()
}

/// One `crate::` path a file names: the line its last segment is on, and its segments after
/// `crate::`.
#[derive(Debug, PartialEq)]
pub(super) struct Import {
    line: usize,
    path: Vec<String>,
}

/// A token of code: a word, or a single other character, with its line.
struct Token<'a> {
    line: usize,
    text: &'a str,
}

/// The words, `::`s and other single characters of `code`, each with its line.
fn tokens(code: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    for (index, line) in code.lines().enumerate() {
        let mut rest = line.trim_start();
        while let Some(first) = rest.chars().next() {
            let word = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let len = if rest.starts_with("::") {
                2
            } else if word > 0 {
                word
            } else {
                first.len_utf8()
            };
            let (text, after) = rest.split_at_checked(len).unwrap_or((rest, ""));
            tokens.push(Token {
                line: index + 1,
                text,
            });
            rest = after.trim_start();
        }
    }
    tokens
}

/// Every `crate::` path in `code`, text `code_only` has cleaned, with a `use` tree flattened to
/// one path per leaf: `self` names its group's own path, an `as` alias is dropped and a glob
/// ends in `*`.
// ponytail: `super::` paths aren't read, so one climbing out of its area isn't seen; nothing
// does that today.
pub(super) fn crate_paths(code: &str) -> Vec<Import> {
    let tokens = tokens(code);
    let mut imports = Vec::new();
    let text = |at: usize| tokens.get(at).map(|token| token.text);
    for at in 0..tokens.len() {
        let after_dollar = at.checked_sub(1).and_then(text) == Some("$");
        if text(at) == Some("crate") && text(at + 1) == Some("::") && !after_dollar {
            let mut next = at + 2;
            use_tree(&tokens, &mut next, &[], &mut imports);
        }
    }
    imports
}

/// Reads the path or `use` tree at `tokens[*at]` under `prefix` into `imports`, moving `at` past
/// it.
fn use_tree(tokens: &[Token<'_>], at: &mut usize, prefix: &[String], imports: &mut Vec<Import>) {
    let Some(token) = tokens.get(*at) else {
        return;
    };
    let text = |index: usize| tokens.get(index).map(|next| next.text);
    *at += 1;
    let mut path = prefix.to_vec();
    match token.text {
        "{" => {
            while let Some(text) = text(*at).filter(|&text| text != "}") {
                if text == "," {
                    *at += 1;
                } else {
                    use_tree(tokens, at, prefix, imports);
                }
            }
            *at += 1;
            return;
        }
        "self" => {}
        // A word, or a glob's `*`.
        word => {
            path.push(word.to_owned());
            if text(*at) == Some("::") && text(*at + 1) != Some("<") {
                *at += 1;
                return use_tree(tokens, at, &path, imports);
            }
            if text(*at) == Some("as") {
                *at += 2;
            }
        }
    }
    imports.push(Import {
        line: token.line,
        path,
    });
}

/// The area or single file the Server source at `path` belongs to: its first part under `src/`,
/// without `.rs` or `_tests`.
pub(super) fn area_of(path: &str) -> Option<&str> {
    let first = path.strip_prefix("src/")?.split('/').next()?;
    let stem = first.strip_suffix(".rs").unwrap_or(first);
    Some(stem.strip_suffix("_tests").unwrap_or(stem))
}

/// Where `module` sits in `rows`: its row and its place in that row, both from 0.
fn place(rows: &[&[&str]], module: &str) -> Option<(usize, usize)> {
    rows.iter().enumerate().find_map(|(row, modules)| {
        modules
            .iter()
            .position(|name| *name == module)
            .map(|column| (row, column))
    })
}

/// Each of `imports`, the `crate::` paths of the file at `path`, that `rows` forbids and
/// `allowed` doesn't let through, as `file:line: import (area, row N -> area, row M)`.
pub(super) fn upward_calls(
    path: &str,
    imports: &[Import],
    rows: &[&[&str]],
    allowed: &[(Importers, &[&str], &str)],
) -> Vec<String> {
    let Some(from) = area_of(path) else {
        return Vec::new();
    };
    let Some((from_row, from_column)) = place(rows, from) else {
        return vec![format!(
            "{path} is in {from}, which has no row: add it to ROWS"
        )];
    };
    let mut problems = Vec::new();
    for import in imports {
        let Some(to) = import.path.first().map(String::as_str) else {
            continue;
        };
        let shown = format!("crate::{}", import.path.join("::"));
        let Some((to_row, to_column)) = place(rows, to) else {
            problems.push(format!("{path}:{}: {shown} ({to} has no row)", import.line));
            continue;
        };
        let down = to_row > from_row || (to_row == from_row && to_column > from_column);
        let let_through = allowed.iter().any(|(importers, leading, _)| {
            let covers = match importers {
                Importers::File(file) => *file == path,
                Importers::TestCode => is_test_file(path),
            };
            covers
                && import.path.len() >= leading.len()
                && leading.iter().zip(&import.path).all(|(a, b)| a == b)
        });
        if to != from && !down && !let_through {
            problems.push(format!(
                "{path}:{}: {shown} ({from}, row {} -> {to}, row {})",
                import.line,
                from_row + 1,
                to_row + 1
            ));
        }
    }
    problems
}

#[test]
fn crate_paths_reads_nested_groups_aliases_self_and_code_paths_but_not_comments() {
    let text = "use crate::{\n    a::{self, B as C},\n    d::{e::{F, G}, *},\n};\n\
                // crate::comment::Line\n/// [`crate::doc::Link`]\n//! crate::inner::Doc\n\
                /* crate::block /* nested */ crate::still::Comment */\n\
                let s = (\"crate::in::Str image/*\", r#\"crate::raw\"#, '\"');\n\
                fn f<'a>() { crate::h::i::<T>(); $crate::in_macro::X; pub(crate) fn g() {} }";
    let paths: Vec<String> = crate_paths(&code_only(text))
        .into_iter()
        .map(|import| format!("{}:{}", import.line, import.path.join("::")))
        .collect();
    assert_eq!(
        paths,
        [
            "2:a",
            "2:a::B",
            "3:d::e::F",
            "3:d::e::G",
            "3:d::*",
            "10:h::i"
        ]
    );
}

#[test]
fn upward_calls_fails_a_call_up_or_left_and_passes_the_rest() {
    let rows: &[&[&str]] = &[&["top"], &["left", "right"], &["bottom"]];
    let allowed: &[(Importers, &[&str], &str)] = &[
        (Importers::File("src/bottom/writes.rs"), &["left"], "why"),
        (Importers::TestCode, &["top", "Builder"], "why"),
    ];
    let calls =
        |path: &str, text: &str| upward_calls(path, &crate_paths(&code_only(text)), rows, allowed);
    assert_eq!(
        calls(
            "src/left/a.rs",
            "use crate::{left::W, right::X, bottom::Y};\nlet z = crate::top::Z;"
        ),
        ["src/left/a.rs:2: crate::top::Z (left, row 2 -> top, row 1)"]
    );
    assert_eq!(
        calls("src/right.rs", "use crate::left;"),
        ["src/right.rs:1: crate::left (right, row 2 -> left, row 2)"]
    );
    assert_eq!(
        calls("src/bottom/a.rs", "use crate::nowhere::X;"),
        ["src/bottom/a.rs:1: crate::nowhere::X (nowhere has no row)"]
    );
    let none = Vec::<String>::new();
    // An allowed module import, called through without `crate::` as decisions.rs does.
    assert_eq!(
        calls("src/bottom/writes.rs", "use crate::{left};\nleft::make();"),
        none
    );
    assert_eq!(calls("src/bottom/other.rs", "use crate::left;").len(), 1);
    assert_eq!(
        calls("src/right_tests.rs", "use crate::top::Builder;"),
        none
    );
    assert_eq!(
        calls("src/left/testing.rs", "use crate::top::Builder;"),
        none
    );
    assert_eq!(
        calls("src/right_tests.rs", "use crate::top::Other;"),
        ["src/right_tests.rs:1: crate::top::Other (right, row 2 -> top, row 1)"]
    );
    assert_eq!(calls("src/right.rs", "use crate::top::Builder;").len(), 1);
}
