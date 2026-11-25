//! The Discord Installed Plugin: posts the Requests the Server tells a User about, and the Links
//! it tells an Admin about, to the Discord webhook that User set in their Personal settings (ADR
//! 0046). The Server builds it for wasm32
//! and bundles it in its binary.

use nama_pdk::{
    Manifest, SettingField, SettingKind, services, setting_default, setting_field, setting_value,
};

const API_URL: &str = "api_url";
const WEBHOOK: &str = "webhook";

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{
        Buffa, FnResult, Manifest, Notification, Notifier, OutsideService, Recipient,
        RecipientGone, Response,
        extism_pdk::{self, Error, HttpRequest, Json},
        nama_plugin_proto::nama::plugin::v1::{
            PersonalSettingsCheck, PersonalSettingsChecked, UserRecipient,
        },
        plugin_fn,
    };
    use serde::{Deserialize, Serialize};

    use super::{API_URL, WEBHOOK};

    const DISCORD: OutsideService = OutsideService::new("Discord").reader(discords_words);

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    /// Refuses a webhook [`super::webhook`] would refuse, so the User learns on saving it, not by
    /// never being notified.
    #[plugin_fn]
    pub fn check_personal_settings(
        Buffa(asked): Buffa<PersonalSettingsCheck>,
    ) -> FnResult<Buffa<PersonalSettingsChecked>> {
        super::webhook(asked.personal_settings.get(WEBHOOK).map(String::as_str))?;
        Ok(Buffa(PersonalSettingsChecked::default()))
    }

    /// Posts each notification for a User to the webhook they set.
    pub(super) struct Discord;

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
        let Some(text) = super::message(notification)? else {
            return Ok(());
        };
        let webhook = super::webhook(user.personal_settings.get(WEBHOOK).map(String::as_str))?;
        let api_url = nama_pdk::required_setting(API_URL)?;
        let url = format!("{}/api/webhooks/{webhook}", api_url.trim_end_matches('/'));
        let request = HttpRequest::new(url)
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        let response = DISCORD.send(&request, Some(Json(Message { content: &text })))?;
        // Discord's "Unknown Webhook": the User deleted it.
        if response.status_code() == 404 {
            return Err(RecipientGone.into());
        }
        DISCORD.accepted(response)?;
        Ok(())
    }

    /// Discord's own words in a refusal: the `message` of its error body.
    fn discords_words(response: &Response) -> Option<String> {
        response
            .json::<Refusal>()
            .ok()
            .map(|refusal| refusal.message)
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

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        logo_svg: include_str!("../logo.svg").to_owned(),
        // No Connector: a User reaches Discord through their Personal settings.
        points: vec![nama_pdk::point(services::NOTIFIER)],
        allowed_hosts: vec!["discord.com".to_owned()],
        settings: vec![
            SettingField {
                default_value: setting_default(setting_value::Value::TextValue(
                    "https://discord.com".to_owned(),
                )),
                ..setting_field(
                    API_URL,
                    "Discord URL",
                    "Where Discord is. Change it only for a proxy or a test.",
                    SettingKind::SETTING_KIND_URL,
                )
            },
            nama_pdk::announce_linking(),
        ],
        personal_settings: vec![SettingField {
            required: true,
            ..setting_field(
                WEBHOOK,
                "Webhook URL",
                "A Discord channel's webhook: in the channel's settings, open Integrations, \
                 then Webhooks, and copy one's URL.",
                SettingKind::SETTING_KIND_SECRET,
            )
        }],
        name: "Discord".to_owned(),
        description: "Tells each User about their Requests, and the Admins about Linking, through \
                      their own Discord webhook."
            .to_owned(),
        ..nama_pdk::manifest("discord", env!("CARGO_PKG_VERSION"))
    }
}

/// What to post about `notification`, then the link to its Request if the Server gives one, or
/// `None` for what a User isn't told.
fn message(
    notification: &nama_pdk::Notification,
) -> Result<Option<String>, nama_pdk::extism_pdk::Error> {
    let told = nama_pdk::announcement(notification)?;
    Ok(told.map(|text| match &notification.request_url {
        Some(link) => format!("{text}\n{link}"),
        None => text,
    }))
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
