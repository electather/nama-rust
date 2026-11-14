use nama_plugin_proto::{
    CONTRACT_VERSION, nama::plugin::v1::Manifest, nama::plugin::v1::SettingField,
};

use super::judge;

#[test]
fn a_manifest_naming_no_points_is_refused_whatever_its_settings() {
    let mut package = Manifest {
        id: "old-plugin".to_owned(),
        version: "0.1.0".to_owned(),
        contract: format!("^{CONTRACT_VERSION}"),
        settings: vec![SettingField {
            key: "api_key".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let refused = judge(&mut package, &[])
        .err()
        .map(|refused| refused.to_string());

    assert_eq!(
        refused.as_deref(),
        Some(
            "its Manifest names no Extension Points, so it would do nothing; a Plugin built \
             before Extension Points became services must name them in points"
        )
    );
}
