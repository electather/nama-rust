use super::service_and_method;

#[test]
fn an_rpc_path_names_its_service_and_method() {
    assert_eq!(
        service_and_method("/nama.hub.v1.HubService/GetHubInfo"),
        Some(("nama.hub.v1.HubService", "GetHubInfo"))
    );
}

#[test]
fn a_path_that_isnt_a_hub_rpc_names_nothing_a_span_would_carry() {
    for path in [
        "/",
        "/favicon.ico",
        "/nama.v1.ServerService/GetServerInfo",
        "/nama.hub.v1.HubService/GetHubInfo/someone@example.com",
    ] {
        assert_eq!(service_and_method(path), None, "{path}");
    }
}
