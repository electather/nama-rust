//! Enforces the import rule in this crate's `AGENTS.md`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Every `.proto` file under `dir`, at any depth.
fn protos(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            found.extend(protos(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "proto") {
            found.push(path);
        }
    }
    Ok(found)
}

/// The files `text` imports, as written between the quotes.
fn imports(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter_map(|line| {
        let rest = line.trim().strip_prefix("import ")?;
        let rest = rest
            .trim_start_matches("public ")
            .trim_start_matches("weak ");
        rest.trim().trim_start_matches('"').split('"').next()
    })
}

#[test]
fn the_contract_imports_only_its_own_files() {
    let contract = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto/nama/plugin");
    let files = protos(&contract).expect("reads the contract's folder");
    assert!(
        !files.is_empty(),
        "read no .proto file under {}",
        contract.display()
    );

    let mut outside = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("reads a proto file");
        outside.extend(
            imports(&text)
                .filter(|imported| !imported.starts_with("nama/plugin/"))
                .map(|imported| format!("{} imports {imported}", path.display())),
        );
    }
    assert!(
        outside.is_empty(),
        "the contract imports outside nama/plugin/:\n{}",
        outside.join("\n")
    );
}
