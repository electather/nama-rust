use std::fs;

use super::{ACCESS, Access};

/// Every RPC `buf generate` wrote a stub for, by path, read from the generated code so a new
/// RPC shows up here without anyone listing it.
fn generated_rpcs() -> Vec<String> {
    let generated = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nama-proto/src/generated/connect"
    );
    let mut rpcs = Vec::new();
    for file in fs::read_dir(generated).expect("the generated code is checked in") {
        let code = fs::read_to_string(file.expect("readable").path()).expect("readable");
        rpcs.extend(
            code.lines()
                .filter_map(|line| line.trim().strip_prefix("\"/nama."))
                .filter_map(|rest| rest.strip_suffix("\","))
                .map(|rest| format!("/nama.{rest}")),
        );
    }
    rpcs
}

#[test]
fn every_generated_rpc_has_an_access_entry() {
    let rpcs = generated_rpcs();
    assert_eq!(rpcs.len(), ACCESS.len(), "the scan found {rpcs:?}");

    let unlisted: Vec<_> = rpcs
        .iter()
        .filter(|rpc| Access::of(rpc).is_none())
        .collect();

    assert!(unlisted.is_empty(), "list these in `ACCESS`: {unlisted:?}");
}

#[test]
fn a_path_in_no_entry_has_no_access() {
    for path in [
        "/nama.v1.LibraryService/Unlisted",
        "/nama.v1.BrowseService/Unlisted",
        "/elsewhere",
    ] {
        assert!(Access::of(path).is_none(), "{path}");
    }
}
