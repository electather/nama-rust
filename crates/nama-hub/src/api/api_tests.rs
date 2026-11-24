use std::sync::Arc;

use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use buffa::Message as _;
use connectrpc::{
    ConnectError, Dispatcher as _, ErrorCode, RequestContext, SpecOrigin,
    client::{CallOptions, ClientConfig, ServiceTransport, call_unary},
};
use nama_hub_proto::nama::hub::v1::{
    __buffa::view::GetHubInfoResponseView, GetHubInfoRequest, Refusal, RefusalReason,
};
use sqlx::postgres::PgPoolOptions;

use super::{Serves, admit, router, serves, service_and_method, services};
use crate::{accounts::Accounts, links::Links, signing::Keys};

/// A signing key, made with `openssl genpkey -algorithm ed25519`.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS
-----END PRIVATE KEY-----
";

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

#[tokio::test]
async fn every_method_refuses_an_anonymous_caller_its_service_doesnt_serve() {
    // Never connects: an anonymous caller is refused before anything asks the database.
    let db = PgPoolOptions::new()
        .connect_lazy("postgres://localhost:1/nowhere")
        .unwrap();
    let keys = Arc::new(Keys::from_pems(SIGNING_KEY).unwrap());
    let accounts = Accounts::new(db.clone(), None, None);
    let links = Links::new(db.clone(), accounts.clone(), Arc::clone(&keys), None);
    let rpcs = services(db.clone(), &accounts, &links);
    let hub = ServiceTransport::new(router(db, keys, &[], None, None, None, None));
    let config = ClientConfig::new("http://hub".parse().unwrap());

    for procedure in rpcs.methods() {
        let (service, _) = procedure.split_once('/').unwrap();
        let reason = match serves(service) {
            None => panic!("{service} is missing from SERVES"),
            Some(Serves::Anyone) => continue,
            Some(Serves::LinkedServer) => RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
            Some(Serves::AccountSession) => RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
        };
        let spec = rpcs
            .lookup(procedure)
            .and_then(|method| method.spec)
            .unwrap();
        // The request is never decoded, so any message does.
        let result = call_unary::<_, _, GetHubInfoResponseView<'static>>(
            &hub,
            &config,
            spec.with_origin(SpecOrigin::Client),
            GetHubInfoRequest::default(),
            CallOptions::default(),
        )
        .await;

        let err = result
            .err()
            .unwrap_or_else(|| panic!("{procedure} served anyone"));
        assert_eq!(err.code, ErrorCode::Unauthenticated, "{procedure}: {err:?}");
        assert_eq!(refusal_of(&err).reason, reason, "{procedure}: {err:?}");
    }
}

#[test]
fn a_service_missing_from_serves_serves_no_one() {
    let ctx = RequestContext::new(HeaderMap::new()).with_path("/nama.hub.v1.NewService/Do");

    assert_eq!(admit(&ctx).unwrap_err().code, ErrorCode::Internal);
}

fn refusal_of(err: &ConnectError) -> Refusal {
    let value = err.details[0].value.as_deref().unwrap();
    Refusal::decode_from_slice(&STANDARD_NO_PAD.decode(value).unwrap()).unwrap()
}
