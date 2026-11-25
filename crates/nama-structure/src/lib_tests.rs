use super::{
    SourceFile, confined_crate_problems, gone_from_rows, is_test_file, named_paths,
    placement_problems, problem, sql_tables, stray_programs, stray_tests, unchecked_queries,
    unsorted_ceilings,
};

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

#[test]
fn sql_tables_reads_the_word_after_a_keyword_across_lines() {
    assert_eq!(
        sql_tables("SELECT x FROM\n    watch_states w JOIN tracks ON from tracks"),
        [("FROM", "watch_states"), ("JOIN", "tracks")]
    );
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

#[test]
fn named_paths_skips_fences_urls_globs_and_bare_names() {
    let text = "`src/a.rs` `/media/` `*.rs` `src/*.rs` `src/{a,b}.rs` `a/<id>` `x::y/z` \
                `README.md` `a b/c`\n```\n`src/fenced.rs`\n```\n`.sqlx/`";
    assert_eq!(named_paths(text), ["src/a.rs", ".sqlx/"]);
}

#[test]
fn placement_problems_names_unplaced_twice_placed_and_gone_modules() {
    let rows: &[&[&str]] = &[&["a"], &["b", "gone", "lib"]];
    let modules = ["lib", "a", "b", "loose"];
    assert_eq!(
        placement_problems(&modules, rows, &["lib"]),
        [
            "src/loose.rs has no row: add it to ROWS",
            "lib is placed twice across STARTUP and ROWS: keep one",
        ]
    );
    assert_eq!(
        gone_from_rows(&modules, rows),
        ["src/gone.rs is gone: remove gone from ROWS, or put back the file"]
    );
    assert_eq!(
        placement_problems(&["lib", "a"], &[&["a"]], &["lib"]),
        Vec::<String>::new()
    );
}

#[test]
fn stray_tests_names_a_test_file_beside_its_folder_at_any_depth() {
    let paths = [
        "src/scanner.rs",
        "src/scanner_tests.rs",
        "src/scanner/scanner_tests.rs",
        "src/db.rs",
        "src/db_tests.rs",
        "src/playback/choices.rs",
        "src/playback/choices_tests.rs",
        "src/playback/choices/preferred_tests.rs",
        "src/playback/autoplay_tests.rs",
    ];
    let folders = ["src/scanner", "src/playback", "src/playback/choices"];
    assert_eq!(
        stray_tests(&paths, &folders),
        [
            "src/scanner_tests.rs sits beside src/scanner/: move it to src/scanner/scanner_tests.rs",
            "src/playback/choices_tests.rs sits beside src/playback/choices/: \
             move it to src/playback/choices/choices_tests.rs",
        ]
    );
}

#[test]
fn unsorted_ceilings_names_the_first_pair_out_of_order_or_twice() {
    assert_eq!(unsorted_ceilings(&[("a.rs", 900), ("b.rs", 900)]), None);
    assert_eq!(
        unsorted_ceilings(&[("b.rs", 900), ("a.rs", 900)]).as_deref(),
        Some("CEILINGS lists b.rs before a.rs: keep it sorted by path with each path once")
    );
    assert!(unsorted_ceilings(&[("a.rs", 900), ("a.rs", 1000)]).is_some());
}

#[test]
fn confined_crate_problems_names_code_outside_the_area_but_not_comments_or_tests() {
    let file = |path: &str, text: &str| SourceFile {
        path: path.to_owned(),
        text: text.to_owned(),
    };
    let files = [
        file("src/api.rs", "use proto::Message;"),
        file("src/api/convert.rs", "use proto::Message;"),
        file(
            "src/accounts.rs",
            "// proto::Message
let a = \"proto\";
use ::proto::Message;",
        ),
        file("src/lib.rs", "fn f() -> proto::Message {}"),
        file("src/links.rs", "use my_proto::Message;"),
        file("tests/accounts.rs", "use proto::Message;"),
    ];
    assert_eq!(
        confined_crate_problems(&files, &[("proto", "api", "why")]),
        [
            "src/accounts.rs:3: names proto outside api (why)",
            "src/lib.rs:1: names proto outside api (why)",
        ]
    );
}

#[test]
fn stray_programs_names_an_unlisted_top_level_test_file() {
    let paths = [
        "src/lib.rs",
        "tests/structure.rs",
        "tests/accounts.rs",
        "tests/hub/main.rs",
        "tests/hub/accounts.rs",
        "tests/support/mod.rs",
    ];
    assert_eq!(
        stray_programs(&paths, &["structure"]),
        [
            "tests/accounts.rs is its own test program, which links the whole package again: \
             make it a module of the package's merged test program"
        ]
    );
    assert_eq!(
        stray_programs(&paths[..2], &["structure"]),
        Vec::<String>::new()
    );
}
