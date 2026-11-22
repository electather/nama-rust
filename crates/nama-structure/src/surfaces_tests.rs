use super::{
    Exception, area_modules, code_only, crate_paths, deep_reaches, rooted, stale_exceptions,
    too_long,
};

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

#[test]
fn too_long_names_a_file_over_30_lines_and_its_length() {
    let thirty = "line\n".repeat(30);
    assert_eq!(too_long("src/a/AGENTS.md", &thirty), None);
    assert_eq!(
        too_long("src/a/AGENTS.md", &format!("{thirty}one more\n")).as_deref(),
        Some("src/a/AGENTS.md is 31 lines, over 30: keep its invariants, then its Neighbours")
    );
}
