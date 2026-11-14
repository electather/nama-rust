//! Keeps `SERVICES` in step with the services `proto/nama/plugin/v1/` declares.

use std::{fs, path::Path};

use nama_plugin_proto::SERVICES;

#[test]
fn services_lists_every_service_the_contract_declares_and_no_other() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto/nama/plugin/v1");
    let mut declared = Vec::new();
    for entry in fs::read_dir(&dir).expect("reads the contract's folder") {
        let text = fs::read_to_string(entry.expect("lists a file").path()).expect("reads a file");
        declared.extend(text.lines().filter_map(|line| {
            let name = line.strip_prefix("service ")?.split_whitespace().next()?;
            Some(format!("nama.plugin.v1.{name}"))
        }));
    }
    declared.sort();
    let mut listed: Vec<_> = SERVICES.iter().map(|&(name, _)| name.to_owned()).collect();
    listed.sort();

    assert_eq!(listed, declared);
}
