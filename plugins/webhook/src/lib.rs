//! The Webhook Installed Plugin: posts every Request event, as JSON, to the URL an Admin sets,
//! signed with their key if they set one. It has no Personal settings or Connector, since a
//! User's own URL would widen the hosts the Plugin reaches (ADR 0046). The Server builds it for
//! wasm32 and bundles it in its binary.

use std::fmt::Write as _;

use hmac::{Hmac, KeyInit as _, Mac as _, digest::InvalidLength};
use nama_pdk::{
    Manifest, Notification, OutsideService, SettingField, SettingKind,
    extism_pdk::json,
    nama_plugin_proto::nama::plugin::v1::{Event, Request},
    services, setting_field,
};
use serde::Serialize;
use sha2::Sha256;

const URL: &str = "url";
const SECRET: &str = "secret";
const WEBHOOK: OutsideService = OutsideService::new("the webhook");

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{
        Buffa, FnResult, Manifest, Notification, Notifier, Recipient,
        extism_pdk::{self, Error, HttpRequest},
        plugin_fn,
    };

    use super::{SECRET, URL, WEBHOOK};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    /// Posts each notification for the channel to the webhook.
    pub(super) struct Webhook;

    impl Notifier for Webhook {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            match recipient {
                Recipient::Channel(_) => post(&notification),
                Recipient::Connection(_) => Err(Error::msg(
                    "Webhook has no Connector, so it can't reach a User by a Plugin Connection",
                )),
                Recipient::User(_) => Err(Error::msg(
                    "Webhook has no Personal settings, so it can't reach a User by them",
                )),
            }
        }
    }

    nama_pdk::notifier!(Webhook);

    /// Posts `notification` to the URL setting, signed with the secret setting if it is set.
    fn post(notification: &Notification) -> Result<(), Error> {
        let Some(text) = nama_pdk::what_happened(notification) else {
            return Ok(());
        };
        let url = nama_pdk::required_setting(URL)?;
        let body = super::body(notification, &text)?;
        let mut request = HttpRequest::new(url)
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        if let Some(secret) = nama_pdk::setting(SECRET)? {
            request = request.with_header("X-Nama-Signature", super::signature(&secret, &body)?);
        }
        WEBHOOK.accepted(WEBHOOK.send(&request, Some(body))?)?;
        Ok(())
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        logo_svg: include_str!("../logo.svg").to_owned(),
        // No `allowed_hosts`: the Server allows the host of the URL setting.
        points: vec![nama_pdk::point(services::NOTIFIER)],
        settings: vec![
            SettingField {
                required: true,
                ..setting_field(
                    URL,
                    "Webhook URL",
                    "Where every Request event is POSTed as JSON.",
                    SettingKind::SETTING_KIND_URL,
                )
            },
            setting_field(
                SECRET,
                "Signing key",
                "Signs each body: the X-Nama-Signature header is sha256= and the hex \
                 HMAC-SHA256 of the body with this key. Leave it empty to sign nothing.",
                SettingKind::SETTING_KIND_SECRET,
            ),
        ],
        name: "Webhook".to_owned(),
        description: "POSTs every Request event as JSON to a URL of your own.".to_owned(),
        ..nama_pdk::manifest("webhook", env!("CARGO_PKG_VERSION"))
    }
}

/// What a webhook gets: the Plugin `Notification`'s fields under their proto names, and `text`.
/// `event` and `request` take protobuf's JSON form, so their own fields are camelCase.
#[derive(Serialize)]
struct Payload<'a> {
    event: Option<&'a Event>,
    request: Option<&'a Request>,
    request_url: Option<&'a str>,
    text: &'a str,
}

/// The JSON body posted about `notification`, saying `text`.
fn body(notification: &Notification, text: &str) -> json::Result<Vec<u8>> {
    let payload = Payload {
        event: notification.event.as_option(),
        request: notification.request.as_option(),
        request_url: notification.request_url.as_deref(),
        text,
    };
    json::to_vec(&payload)
}

/// The `X-Nama-Signature` of `body` signed with `secret`: `sha256=` and its hex HMAC-SHA256.
/// HMAC takes a key of any length, so it never fails.
fn signature(secret: &str, body: &[u8]) -> Result<String, InvalidLength> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
    mac.update(body);
    let signed = mac.finalize().into_bytes();
    Ok(signed
        .iter()
        .fold(String::from("sha256="), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        }))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
