//! `HubService.GetHubInfo`, the Hub's health check, against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;

use connectrpc::{
    ErrorCode, SpecOrigin,
    client::{CallOptions, HttpClient, call_unary_get},
};
use hub::TestHub;
use nama_hub_proto::nama::hub::v1::{
    __buffa::view::GetHubInfoResponseView, GetHubInfoRequest, HUB_SERVICE_GET_HUB_INFO_SPEC,
    HubServiceClient,
};
use sqlx::PgPool;

fn client(hub: &TestHub) -> HubServiceClient<HttpClient> {
    HubServiceClient::new(HttpClient::plaintext(), hub.client_config())
}

#[sqlx::test(migrations = false)]
async fn get_hub_info_answers_the_hubs_version_without_sign_in(db: PgPool) {
    let hub = TestHub::start(db, None).await;

    let info = client(&hub)
        .get_hub_info(GetHubInfoRequest::default())
        .await
        .unwrap()
        .into_owned();

    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
}

#[sqlx::test(migrations = false)]
async fn get_hub_info_answers_a_get_request_so_a_host_can_probe_it(db: PgPool) {
    let hub = TestHub::start(db, None).await;

    let info = call_unary_get::<_, _, GetHubInfoResponseView<'static>>(
        &HttpClient::plaintext(),
        &hub.client_config(),
        HUB_SERVICE_GET_HUB_INFO_SPEC.with_origin(SpecOrigin::Client),
        GetHubInfoRequest::default(),
        CallOptions::default(),
    )
    .await
    .unwrap()
    .into_owned();

    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
}

#[sqlx::test(migrations = false)]
async fn get_hub_info_answers_unavailable_while_postgres_is_down(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    hub.lose_database().await;

    let err = client(&hub)
        .get_hub_info(GetHubInfoRequest::default())
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::Unavailable);
}
