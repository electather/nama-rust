//! The Telegram Installed Plugin: posts every Request event to a Telegram channel, connects Users
//! to their own Telegram chat with the bot through a deep link, and messages them there about the
//! Requests the Server tells them of (ADR 0021). The Server builds it for wasm32 and bundles it in
//! its binary.

use nama_pdk::{
    Manifest, Notification, SettingField, SettingKind, services, setting_default, setting_field,
    setting_value,
};

const BOT_TOKEN: &str = "bot_token";
const CHANNEL: &str = "channel";
const API_URL: &str = "api_url";

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::collections::BTreeMap;

    use nama_pdk::{
        Buffa, ConnectionStarted, Connector, FnResult, Manifest, Notification, Notifier,
        OutsideService, Recipient, RecipientGone, Refusal, Response,
        extism_pdk::{self, Error, HttpRequest, Json},
        plugin_fn,
    };
    use serde::{
        Deserialize, Serialize,
        de::{DeserializeOwned, IgnoredAny},
    };

    use super::{API_URL, BOT_TOKEN, CHANNEL};

    const TELEGRAM: OutsideService = OutsideService::new("Telegram")
        .credential("bot token")
        .reader(telegrams_words);

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    /// Posts each notification for the channel to it, if one is set, and sends each one for a
    /// User to the chat their Plugin Connection names.
    pub(super) struct Telegram;

    impl Notifier for Telegram {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            let Some(text) = super::message(&notification) else {
                return Ok(());
            };
            match recipient {
                Recipient::Channel(_) => nama_pdk::setting(CHANNEL)?
                    .map_or(Ok(()), |chat| send_message(chat.trim(), &text)),
                // Telegram refuses a User's own chat with a 403 once they block the bot.
                Recipient::Connection(chat_id) => match send_message(&chat_id, &text) {
                    Err(error) if blocked(&error) => Err(RecipientGone.into()),
                    sent => sent,
                },
                // Telegram declares no Personal settings, so a User sent this way has nothing to
                // be reached by.
                Recipient::User(_) => Err(Error::msg(
                    "Telegram has no Personal settings, so it can't reach a User by them",
                )),
            }
        }
    }

    /// Whether `error` is Telegram's 403: the bot may not post in that chat, as when its User
    /// blocked the bot or a channel didn't make it an admin.
    fn blocked(error: &Error) -> bool {
        error
            .downcast_ref::<Refusal>()
            .is_some_and(|refusal| refusal.status() == 403)
    }

    nama_pdk::notifier!(Telegram);

    /// Connects a User through a deep link to the bot, which sends `/start <code>` from their own
    /// chat when they press Start. Polled with `getUpdates`, since a webhook needs the Server on a
    /// public domain.
    impl Connector for Telegram {
        fn start_connection(code: String) -> Result<ConnectionStarted, Error> {
            let bot: Bot = call("getMe", BTreeMap::<&str, &str>::new())?;
            Ok(ConnectionStarted {
                url: format!("https://t.me/{}?start={code}", bot.username),
                pending: code,
                ..ConnectionStarted::default()
            })
        }

        fn poll_connection(pending: String) -> Result<Option<String>, Error> {
            // The last 100 updates, forgetting older ones, so the queue never fills: a pending
            // code is at most 10 minutes old, and the bot reads its messages for nothing else.
            let updates: Vec<Update> = call("getUpdates", GetUpdates { offset: -100 })?;
            let start = format!("/start {pending}");
            let started = updates
                .into_iter()
                .filter_map(|update| update.message)
                .find(|message| message.text.as_deref() == Some(start.as_str()));
            Ok(started.map(|message| message.chat.id.to_string()))
        }
    }

    nama_pdk::connector!(Telegram);

    /// Posts `text` to the chat `chat_id` with the Bot API's `sendMessage`.
    fn send_message(chat_id: &str, text: &str) -> Result<(), Error> {
        call::<IgnoredAny>("sendMessage", SendMessage { chat_id, text }).map(drop)
    }

    /// Calls the Bot API's `method` with `parameters`, and returns its result.
    fn call<T: DeserializeOwned>(method: &str, parameters: impl Serialize) -> Result<T, Error> {
        let api_url = nama_pdk::required_setting(API_URL)?;
        let token = nama_pdk::required_setting(BOT_TOKEN)?;
        let url = format!("{}/bot{token}/{method}", api_url.trim_end_matches('/'));
        let request = HttpRequest::new(url)
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        let response = TELEGRAM.send(&request, Some(Json(parameters)))?;
        // Telegram answers 404 to a token that isn't even shaped like one.
        if response.status_code() == 404 {
            return Err(Error::msg("Telegram refused the bot token"));
        }
        TELEGRAM
            .accepted(response)?
            .json::<Answer<T>>()
            .map(|answer| answer.result)
            .map_err(|err| Error::msg(format!("Telegram's answer made no sense: {err}")))
    }

    /// Telegram's own words in a refusal: the `description` of a Bot API error.
    fn telegrams_words(response: &Response) -> Option<String> {
        response
            .json::<ApiError>()
            .ok()
            .map(|error| error.description)
    }

    /// What `sendMessage` takes. Plain text, so a title needs no escaping.
    #[derive(Serialize)]
    struct SendMessage<'a> {
        chat_id: &'a str,
        text: &'a str,
    }

    /// What `getUpdates` takes.
    #[derive(Serialize)]
    struct GetUpdates {
        offset: i64,
    }

    /// The body of a Bot API success.
    #[derive(Deserialize)]
    struct Answer<T> {
        result: T,
    }

    /// The bot, as `getMe` gives it.
    #[derive(Deserialize)]
    struct Bot {
        username: String,
    }

    /// One update `getUpdates` gives.
    #[derive(Deserialize)]
    struct Update {
        message: Option<Message>,
    }

    /// A message someone sent the bot.
    #[derive(Deserialize)]
    struct Message {
        chat: Chat,
        text: Option<String>,
    }

    /// The chat a message was sent in.
    #[derive(Deserialize)]
    struct Chat {
        id: i64,
    }

    /// The body of a Bot API error.
    #[derive(Deserialize)]
    struct ApiError {
        description: String,
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![
            nama_pdk::point(services::NOTIFIER),
            nama_pdk::point(services::CONNECTOR),
        ],
        allowed_hosts: vec!["api.telegram.org".to_owned()],
        settings: vec![
            SettingField {
                required: true,
                ..setting_field(
                    BOT_TOKEN,
                    "Bot token",
                    "The token BotFather gave your bot, such as 123456:ABC-DEF1234ghIkl.",
                    SettingKind::SETTING_KIND_SECRET,
                )
            },
            setting_field(
                CHANNEL,
                "Channel",
                "Where every Request event is posted: a chat id, such as -1001234567890, or a \
                 public channel's @name. The bot must be an admin there. Leave it empty to \
                 post nothing.",
                SettingKind::SETTING_KIND_TEXT,
            ),
            SettingField {
                default_value: setting_default(setting_value::Value::TextValue(
                    "https://api.telegram.org".to_owned(),
                )),
                ..setting_field(
                    API_URL,
                    "Bot API URL",
                    "Where the Telegram Bot API is. Change it only for a Bot API server of \
                     your own.",
                    SettingKind::SETTING_KIND_URL,
                )
            },
        ],
        name: "Telegram".to_owned(),
        description: "Posts Requests to a Telegram channel and tells Users about theirs in a chat."
            .to_owned(),
        ..nama_pdk::manifest("telegram", env!("CARGO_PKG_VERSION"))
    }
}

/// What to post about `notification`, in the Glossary's words, then the link to its Request if
/// the Server gives one, or `None` for what the channel isn't told.
fn message(notification: &Notification) -> Option<String> {
    let text = nama_pdk::what_happened(notification)?;
    Some(match &notification.request_url {
        Some(link) => format!("{text}\n{link}"),
        None => text,
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
