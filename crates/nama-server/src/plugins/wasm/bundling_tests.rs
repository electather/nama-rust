use super::bundles;

#[test]
fn a_cdylib_is_bundled() {
    let cargo_toml = "[package]\nname = \"nama-plugin-x\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n";

    assert!(bundles(cargo_toml));
}

#[test]
fn a_cdylib_that_says_bundled_false_is_not_bundled() {
    let cargo_toml = "[package]\nname = \"nama-plugin-x\"\n\n[package.metadata.nama]\n\
                      bundled = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n";

    assert!(!bundles(cargo_toml));
}

#[test]
fn an_rlib_is_not_bundled() {
    let cargo_toml = "[package]\nname = \"nama-plugin-x\"\n\n[lib]\ncrate-type = [\"rlib\"]\n";

    assert!(!bundles(cargo_toml));
}
