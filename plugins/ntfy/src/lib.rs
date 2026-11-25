//! The ntfy Installed Plugin: publishes the Requests the Server tells a User about, and the Links
//! it tells an Admin about, to the ntfy topic that User set in their Personal settings (ADR
//! 0046), on ntfy.sh or an ntfy server the Admin names. The Server builds it for wasm32 and
//! bundles it in its binary.

use nama_pdk::{
    Manifest, SettingField, SettingKind,
    nama_plugin_proto::nama::plugin::v1::PersonalSettingsCheck, services, setting_default,
    setting_field, setting_value,
};

const SERVER_URL: &str = "server_url";
const TOPIC: &str = "topic";
const ACCESS_TOKEN: &str = "access_token";

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{
        Buffa, FnResult, Manifest, Notification, Notifier, OutsideService, Recipient, Response,
        extism_pdk::{self, Error, HttpRequest},
        nama_plugin_proto::nama::plugin::v1::{
            PersonalSettingsCheck, PersonalSettingsChecked, UserRecipient,
        },
        plugin_fn,
    };
    use serde::Deserialize;

    use super::{ACCESS_TOKEN, SERVER_URL, TOPIC};

    const NTFY: OutsideService = OutsideService::new("ntfy")
        .credential("access token")
        .reader(ntfys_words);

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    #[plugin_fn]
    pub fn check_personal_settings(
        Buffa(asked): Buffa<PersonalSettingsCheck>,
    ) -> FnResult<Buffa<PersonalSettingsChecked>> {
        super::check_personal_settings(&asked)?;
        Ok(Buffa(PersonalSettingsChecked::default()))
    }

    /// Publishes each notification for a User to the topic they set.
    pub(super) struct Ntfy;

    impl Notifier for Ntfy {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            match recipient {
                // ntfy has no channel: every topic is one User's.
                Recipient::Channel(_) => Ok(()),
                // ntfy declares no Connector, so a User has no Plugin Connection to it.
                Recipient::Connection(_) => Err(Error::msg(
                    "ntfy has no Connector, so it can't reach a User by a Plugin Connection",
                )),
                Recipient::User(user) => publish(&notification, &user),
            }
        }
    }

    nama_pdk::notifier!(Ntfy);

    /// Publishes `notification` to the topic `user` set, with its access token if set. ntfy
    /// has no answer meaning a topic is gone, so this never fails with `RecipientGone`.
    fn publish(notification: &Notification, user: &UserRecipient) -> Result<(), Error> {
        let personal = &user.personal_settings;
        let Some(text) = nama_pdk::announcement(notification)? else {
            return Ok(());
        };
        let topic = super::topic(personal.get(TOPIC).map(String::as_str))?;
        let server_url = nama_pdk::required_setting(SERVER_URL)?;
        let mut request = HttpRequest::new(format!("{}/{topic}", server_url.trim_end_matches('/')))
            .with_method("POST")
            .with_header("Title", "Nama");
        if let Some(link) = &notification.request_url {
            request = request.with_header("Click", link);
        }
        if let Some(token) = personal.get(ACCESS_TOKEN).filter(|token| !token.is_empty()) {
            request = request.with_header("Authorization", format!("Bearer {token}"));
        }
        NTFY.accepted(NTFY.send(&request, Some(text))?)?;
        Ok(())
    }

    /// ntfy's own words in a refusal: the `error` of its error body.
    fn ntfys_words(response: &Response) -> Option<String> {
        response.json::<Refusal>().ok()?.error
    }

    /// The body of an ntfy error, such as `{"code":40101,"http":401,"error":"unauthorized"}`.
    #[derive(Deserialize)]
    struct Refusal {
        error: Option<String>,
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        logo_svg: include_str!("../logo.svg").to_owned(),
        // No `allowed_hosts`: the Server allows the host of the ntfy server setting.
        points: vec![nama_pdk::point(services::NOTIFIER)],
        settings: vec![
            SettingField {
                default_value: setting_default(setting_value::Value::TextValue(
                    "https://ntfy.sh".to_owned(),
                )),
                ..setting_field(
                    SERVER_URL,
                    "ntfy server",
                    "The ntfy server every User's topic is on. Change it only for an ntfy server \
                     of your own.",
                    SettingKind::SETTING_KIND_URL,
                )
            },
            nama_pdk::announce_linking(),
        ],
        // The token is each User's, since the topic it opens is theirs.
        personal_settings: vec![
            SettingField {
                required: true,
                ..setting_field(
                    TOPIC,
                    "Topic",
                    "The ntfy topic you subscribe to, 1 to 64 letters, digits, _ or -. \
                     Anyone who knows it can read it, so pick one hard to guess.",
                    SettingKind::SETTING_KIND_TEXT,
                )
            },
            setting_field(
                ACCESS_TOKEN,
                "Access token",
                "An ntfy access token, for a protected topic. Leave it empty for an open one.",
                SettingKind::SETTING_KIND_SECRET,
            ),
        ],
        name: "ntfy".to_owned(),
        description:
            "Tells each User about their Requests, and the Admins about Linking, on their \
                      own ntfy topic."
                .to_owned(),
        ..nama_pdk::manifest("ntfy", env!("CARGO_PKG_VERSION"))
    }
}

/// Refuses a User's Personal settings as [`topic`] refuses their topic, so they learn on saving it,
/// not by never being notified.
fn check_personal_settings(
    asked: &PersonalSettingsCheck,
) -> Result<(), nama_pdk::extism_pdk::Error> {
    topic(asked.personal_settings.get(TOPIC).map(String::as_str)).map(drop)
}

/// `topic` if ntfy takes it: 1 to 64 letters, digits, `_` or `-`. A failure names the setting.
fn topic(topic: Option<&str>) -> Result<&str, nama_pdk::extism_pdk::Error> {
    match topic {
        Some(topic)
            if (1..=64).contains(&topic.len())
                && topic
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) =>
        {
            Ok(topic)
        }
        Some(_) => Err(nama_pdk::extism_pdk::Error::msg(
            "the Personal setting topic must be 1 to 64 letters, digits, _ or -",
        )),
        None => Err(nama_pdk::extism_pdk::Error::msg(
            "the Personal setting topic isn't set",
        )),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
