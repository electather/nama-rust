//! The spike's Installed Plugin: one export per question the host asks of Extism.

use extism_pdk::{FnResult, HttpRequest, http, plugin_fn};
use shared::Buffa;
use shared::spike::v1::{FetchRequest, FetchResponse, GreetRequest, GreetResponse};

const VERSION: &str = if cfg!(feature = "v2") { "v2" } else { "v1" };

/// Protobuf in, protobuf out: the round trip and the call-cost probe.
#[plugin_fn]
pub fn greet(Buffa(request): Buffa<GreetRequest>) -> FnResult<Buffa<GreetResponse>> {
    Ok(Buffa(GreetResponse {
        message: format!("Hello, {}!", request.name),
        guest_version: VERSION.into(),
        ..Default::default()
    }))
}

/// GETs a URL through Extism's HTTP host function, which enforces the manifest's allowed hosts.
#[plugin_fn]
pub fn fetch(Buffa(request): Buffa<FetchRequest>) -> FnResult<Buffa<FetchResponse>> {
    let response = http::request::<()>(&HttpRequest::new(request.url), None)?;
    Ok(Buffa(FetchResponse {
        status: response.status_code().into(),
        body: String::from_utf8_lossy(&response.body()).into_owned(),
        ..Default::default()
    }))
}

/// Never returns, so the host can check that its timeout stops it.
#[plugin_fn]
pub fn spin() -> FnResult<()> {
    #[expect(clippy::empty_loop, reason = "looping forever is the point")]
    loop {}
}

/// Panics, which traps the WASM instance, so the host can check it survives.
#[plugin_fn]
pub fn crash() -> FnResult<()> {
    panic!("the guest panics on purpose");
}
