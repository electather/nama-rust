use super::{Allowed, Importers, code_only, crate_paths, stale_allowances, upward_calls};

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
            "10:h::i",
            "10:in_macro::X"
        ]
    );
}

#[test]
fn upward_calls_fails_a_call_up_or_left_and_passes_the_rest() {
    let rows: &[&[&str]] = &[&["top"], &["left", "right"], &["bottom"]];
    let allowed: &[Allowed] = &[
        (
            Importers::File("src/bottom/writes.rs"),
            &["left", "make"],
            "why writes",
        ),
        (Importers::TestCode, &["top", "Builder"], "why tests"),
        (Importers::File("src/gone.rs"), &["top"], "why gone"),
    ];
    let mut used = vec![false; allowed.len()];
    let mut calls = |path: &str, text: &str| {
        upward_calls(
            path,
            &crate_paths(&code_only(text)),
            rows,
            allowed,
            &mut used,
        )
    };
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
    assert_eq!(
        calls("src/bottom/writes.rs", "use crate::left::make;\nmake();"),
        none
    );
    assert_eq!(
        calls("src/bottom/writes.rs", "use crate::left;\nleft::make();"),
        [
            "src/bottom/writes.rs:1: crate::left (bottom, row 3 -> left, row 2)\n    \
             allowed here only: crate::left::make (why writes)"
        ]
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
        [
            "src/right_tests.rs:1: crate::top::Other (right, row 2 -> top, row 1)\n    \
             allowed here only: crate::top::Builder (why tests)"
        ]
    );
    assert_eq!(calls("src/right.rs", "use crate::top::Builder;").len(), 1);
    assert_eq!(
        stale_allowances(allowed, &used),
        [
            "ALLOWED_UPWARD lets src/gone.rs call crate::top, but nothing does: remove the entry \
          (why gone)"
        ]
    );
}
