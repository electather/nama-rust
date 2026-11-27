use nama_hub_proto::nama::hub::v1::TitleKind;

use super::watchlisted_to_proto;
use crate::{sync::WatchlistedTitle, titles::ExternalId};

#[test]
fn a_watchlisted_show_reaches_a_server_as_its_kind_external_id_and_weight() {
    let watchlisted = watchlisted_to_proto(WatchlistedTitle {
        show: true,
        id: ExternalId::parse("tvdb-81189").unwrap(),
        weight: 0.5,
    });

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
