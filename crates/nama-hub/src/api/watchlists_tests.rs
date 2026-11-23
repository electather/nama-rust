use nama_hub_proto::nama::hub::v1::TitleKind;

use super::watchlisted_to_proto;
use crate::sync::WatchlistedTitle;

#[test]
fn a_watchlisted_show_reaches_a_server_as_its_kind_external_id_and_weight() {
    let watchlisted = watchlisted_to_proto(&WatchlistedTitle {
        show: true,
        external_id: "tvdb-81189".to_owned(),
        weight: 0.5,
    })
    .unwrap();

    assert_eq!(
        watchlisted.kind.as_known(),
        Some(TitleKind::TITLE_KIND_SHOW)
    );
    let id = watchlisted.id.as_option().unwrap();
    assert_eq!(
        (id.namespace.as_str(), id.value.as_str()),
        ("tvdb", "81189")
    );
    assert!((watchlisted.weight - 0.5).abs() < f32::EPSILON);
}

#[test]
fn a_watchlisted_id_without_a_namespace_is_left_out() {
    let watchlisted = watchlisted_to_proto(&WatchlistedTitle {
        show: false,
        external_id: "603".to_owned(),
        weight: 1.0,
    });

    assert!(watchlisted.is_none());
}
