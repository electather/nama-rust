//! The Discord Installed Plugin: posts the Requests the Server tells a User about to the Discord
//! webhook that User set in their Personal settings (ADR 0046). The Server builds it for wasm32
//! and bundles it in its binary.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{
        Buffa, FnResult, Manifest, Notification, Notifier, Recipient, RecipientGone,
        extism_pdk::{self, Error, HttpRequest, HttpResponse, Json, config},
        nama_plugin_proto::nama::plugin::v1::{
            SettingField, SettingKind, SettingValue, UserRecipient, setting_value,
        },
        plugin_fn, services,
    };
    use serde::{Deserialize, Serialize};

    const API_URL: &str = "api_url";
    const WEBHOOK: &str = "webhook";

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        let setting = |key: &str, label: &str, description: &str, kind: SettingKind| SettingField {
            key: key.to_owned(),
            label: label.to_owned(),
            description: description.to_owned(),
            kind: kind.into(),
            ..SettingField::default()
        };
        Ok(Buffa(Manifest {
            logo_svg: include_str!("../logo.svg").to_owned(),
            // No Connector: a User reaches Discord through their Personal settings.
            points: vec![nama_pdk::point(services::NOTIFIER)],
            allowed_hosts: vec!["discord.com".to_owned()],
            settings: vec![SettingField {
                default_value: SettingValue {
                    value: Some(setting_value::Value::TextValue(
                        "https://discord.com".to_owned(),
                    )),
                    ..SettingValue::default()
                }
                .into(),
                ..setting(
                    API_URL,
                    "Discord URL",
                    "Where Discord is. Change it only for a proxy or a test.",
                    SettingKind::SETTING_KIND_URL,
                )
            }],
            personal_settings: vec![SettingField {
                required: true,
                ..setting(
                    WEBHOOK,
                    "Webhook URL",
                    "A Discord channel's webhook: in the channel's settings, open Integrations, \
                     then Webhooks, and copy one's URL.",
                    SettingKind::SETTING_KIND_SECRET,
                )
            }],
            name: "Discord".to_owned(),
            description: "Tells each User about their Requests through their own Discord webhook."
                .to_owned(),
            ..nama_pdk::manifest("discord", env!("CARGO_PKG_VERSION"))
        }))
    }

    /// Posts each notification for a User to the webhook they set.
    struct Discord;

    impl Notifier for Discord {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            match recipient {
                // Discord has no channel of its own here: every webhook is one User's.
                Recipient::Channel(_) => Ok(()),
                // Discord declares no Connector, so a User has no Plugin Connection to it.
                Recipient::Connection(_) => Err(Error::msg(
                    "Discord has no Connector, so it can't reach a User by a Plugin Connection",
                )),
                Recipient::User(user) => post(&notification, &user),
            }
        }
    }

    nama_pdk::notifier!(Discord);

    /// Posts `notification` to the webhook `user` set, on the Discord URL setting, so a User's
    /// value never widens the hosts the Plugin reaches (ADR 0046).
    fn post(notification: &Notification, user: &UserRecipient) -> Result<(), Error> {
        let Some(text) = super::message(notification) else {
            return Ok(());
        };
        let webhook = super::webhook(user.personal_settings.get(WEBHOOK).map(String::as_str))?;
        let api_url = config::get(API_URL)?
            .ok_or_else(|| Error::msg(format!("the setting {API_URL} isn't set")))?;
        let url = format!("{}/api/webhooks/{webhook}", api_url.trim_end_matches('/'));
        let request = HttpRequest::new(url)
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        // Not naming the URL, which holds the webhook's token.
        let response = nama_pdk::http_request(
            &request,
            Some(Json(Message { content: &text })),
            "Discord didn't answer",
        )?;
        match response.status_code() {
            200..300 => Ok(()),
            // Discord's "Unknown Webhook": the User deleted it.
            404 => Err(RecipientGone.into()),
            _ => Err(refused(&response)),
        }
    }

    /// Why Discord refused a message: its status, and its own words if it gave any.
    fn refused(response: &HttpResponse) -> Error {
        let status = response.status_code();
        match response.json::<Refusal>() {
            Ok(Refusal { message }) => Error::msg(format!(
                "Discord failed with HTTP status {status}: {message}"
            )),
            Err(_) => Error::msg(format!("Discord failed with HTTP status {status}")),
        }
    }

    /// What a webhook takes. Plain text, so a title needs no escaping.
    #[derive(Serialize)]
    struct Message<'a> {
        content: &'a str,
    }

    /// The body of a Discord error, such as `{"message": "Unknown Webhook", "code": 10015}`.
    #[derive(Deserialize)]
    struct Refusal {
        message: String,
    }
}

/// What to post about `notification`, then the link to its Request if the Server gives one, or
/// `None` for what a User isn't told.
fn message(notification: &nama_pdk::Notification) -> Option<String> {
    let text = nama_pdk::what_happened(notification)?;
    Some(match &notification.request_url {
        Some(link) => format!("{text}\n{link}"),
        None => text,
    })
}

/// The `<id>/<token>` after `/api/webhooks/` in `webhook`, whatever its host. A failure names the
/// setting but never its value, which is a secret.
fn webhook(webhook: Option<&str>) -> Result<String, nama_pdk::extism_pdk::Error> {
    let refused = || {
        nama_pdk::extism_pdk::Error::msg(
            "the Personal setting webhook must be a Discord webhook URL, with /api/webhooks/ in it",
        )
    };
    let webhook = webhook.ok_or_else(|| {
        nama_pdk::extism_pdk::Error::msg("the Personal setting webhook isn't set")
    })?;
    let (_, path) = webhook.split_once("/api/webhooks/").ok_or_else(refused)?;
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let mut parts = path.trim_end_matches('/').split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(id), Some(token), None)
            if !id.is_empty()
                && id.bytes().all(|byte| byte.is_ascii_digit())
                && !token.is_empty()
                && token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) =>
        {
            Ok(format!("{id}/{token}"))
        }
        _ => Err(refused()),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
