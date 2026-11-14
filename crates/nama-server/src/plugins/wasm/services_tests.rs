use nama_plugin_proto::SERVICES;

use super::POINTS;

#[test]
fn every_contract_service_has_adapter_rows_and_every_row_a_contract_service() {
    for (service, _) in SERVICES {
        let point = POINTS.iter().find(|point| point.service == *service);
        assert!(
            point.is_some_and(|point| !point.methods.is_empty()),
            "{service} has no rows"
        );
    }
    for point in POINTS {
        assert!(
            SERVICES
                .iter()
                .any(|&(service, _)| service == point.service),
            "{} isn't a contract service",
            point.service
        );
        for method in point.methods {
            assert_eq!(method.service, point.service, "{}", method.name);
        }
    }
}
