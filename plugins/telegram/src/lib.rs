//! The Telegram Installed Plugin: posts every Request event to a Telegram channel, connects Users
//! to their own Telegram chat with the bot through a deep link, and messages them there about the
//! Requests the Server tells them of (ADR 0021). The Server builds it for wasm32 and bundles it in
//! its binary.

use nama_pdk::{
    Notification,
    nama_proto::nama::v1::{Request, RequestState, event::Event},
};

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::collections::BTreeMap;

    use nama_pdk::{
        Buffa, ConnectionStarted, Connector, FnResult, Manifest, Notification, Notifier, Recipient,
        extism_pdk::{self, Error, HttpRequest, HttpResponse, Json, config},
        nama_proto::nama::plugin::v1::{
            ProvidePoint, SettingField, SettingKind, SettingValue, setting_value,
        },
        plugin_fn,
    };
    use serde::{
        Deserialize, Serialize,
        de::{DeserializeOwned, IgnoredAny},
    };

    const BOT_TOKEN: &str = "bot_token";
    const CHANNEL: &str = "channel";
    const API_URL: &str = "api_url";

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
            providers: vec![
                ProvidePoint::PROVIDE_POINT_NOTIFIER.into(),
                ProvidePoint::PROVIDE_POINT_CONNECTOR.into(),
            ],
            allowed_hosts: vec!["api.telegram.org".to_owned()],
            settings: vec![
                SettingField {
                    required: true,
                    ..setting(
                        BOT_TOKEN,
                        "Bot token",
                        "The token BotFather gave your bot, such as 123456:ABC-DEF1234ghIkl.",
                        SettingKind::SETTING_KIND_SECRET,
                    )
                },
                setting(
                    CHANNEL,
                    "Channel",
                    "Where every Request event is posted: a chat id, such as -1001234567890, or a \
                     public channel's @name. The bot must be an admin there. Leave it empty to \
                     post nothing.",
                    SettingKind::SETTING_KIND_TEXT,
                ),
                SettingField {
                    default_value: SettingValue {
                        value: Some(setting_value::Value::Text(
                            "https://api.telegram.org".to_owned(),
                        )),
                        ..SettingValue::default()
                    }
                    .into(),
                    ..setting(
                        API_URL,
                        "Bot API URL",
                        "Where the Telegram Bot API is. Change it only for a Bot API server of \
                         your own.",
                        SettingKind::SETTING_KIND_URL,
                    )
                },
            ],
            ..nama_pdk::manifest("telegram", env!("CARGO_PKG_VERSION"))
        }))
    }

    /// Posts each notification for the channel to it, if one is set, and sends each one for a
    /// User to the chat their Plugin Connection names.
    struct Telegram;

    impl Notifier for Telegram {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            let chat = match recipient {
                Recipient::Channel(_) => channel()?,
                Recipient::Connection(chat_id) => Some(chat_id),
            };
            match (chat, super::message(&notification)) {
                (Some(chat), Some(text)) => send_message(&chat, &text),
                _ => Ok(()),
            }
        }
    }

    /// The channel setting, or `None` if it is empty.
    fn channel() -> Result<Option<String>, Error> {
        let channel = config::get(CHANNEL)?;
        Ok(channel
            .map(|channel| channel.trim().to_owned())
            .filter(|channel| !channel.is_empty()))
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
        let api_url = setting(API_URL)?;
        let token = setting(BOT_TOKEN)?;
        let url = format!("{}/bot{token}/{method}", api_url.trim_end_matches('/'));
        let request = HttpRequest::new(url)
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        // Not naming the URL, which holds the token.
        let response =
            nama_pdk::http_request(&request, Some(Json(parameters)), "Telegram didn't answer")?;
        if !(200..300).contains(&response.status_code()) {
            return Err(refused(&response));
        }
        Ok(response.json::<Answer<T>>()?.result)
    }

    /// The setting `key`, which the Server sets before every call.
    fn setting(key: &str) -> Result<String, Error> {
        config::get(key)?.ok_or_else(|| Error::msg(format!("the setting {key} isn't set")))
    }

    /// Why Telegram refused a request, in words an admin understands.
    fn refused(response: &HttpResponse) -> Error {
        let reason = match response.status_code() {
            // Telegram answers 404 to a token that isn't even shaped like one.
            401 | 404 => "Telegram refused the bot token".to_owned(),
            status => match response.json::<Refusal>() {
                Ok(refusal) => format!("Telegram said: {}", refusal.description),
                Err(_) => format!("Telegram failed with HTTP status {status}"),
            },
        };
        Error::msg(reason)
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
    struct Refusal {
        description: String,
    }
}

/// What to post about `notification`, in the Glossary's words, then the link to its Request if
/// the Server gives one, or `None` for what the channel isn't told.
fn message(notification: &Notification) -> Option<String> {
    let text = what_happened(notification)?;
    Some(match &notification.request_url {
        Some(link) => format!("{text}\n{link}"),
        None => text,
    })
}

/// What happened, or `None` for what the channel isn't told: download progress, which changes
/// every minute, and a state this Plugin doesn't know.
fn what_happened(notification: &Notification) -> Option<String> {
    let request = notification.request.as_option()?;
    let title = title(request);
    let name = |user_id: i64| {
        request
            .requesters
            .iter()
            .find(|requester| requester.user_id == user_id)
            .map_or("Someone", |requester| requester.name.as_str())
    };
    Some(match notification.event.as_option()?.event.as_ref()? {
        Event::RequestCreated(_) => {
            let asker = request
                .requesters
                .first()
                .map_or("someone", |requester| requester.name.as_str());
            format!(
                "New Request: {title}, from {asker}, is {}.",
                state(request)?
            )
        }
        Event::RequesterAdded(added) => format!("{} also asked for {title}.", name(added.user_id)),
        Event::RequestStateChanged(_) => {
            let reason = request
                .failure_reason
                .as_ref()
                .or(request.decline_reason.as_ref());
            let reason = reason.map_or_else(String::new, |reason| format!(" Reason: {reason}"));
            format!("{title} is now {}.{reason}", state(request)?)
        }
        Event::RequestCancelled(_) => format!("The Request for {title} was cancelled."),
        Event::RequesterRemoved(removed) => format!(
            "{} withdrew from the Request for {title}.",
            name(removed.user_id)
        ),
        Event::RequestWithdrawn(_) => format!("The Request for {title} was withdrawn."),
        Event::RequestProgress(_)
        | Event::ScanStarted(_)
        | Event::ScanProgress(_)
        | Event::ScanFinished(_)
        | Event::LibraryItemAdded(_)
        | Event::LibraryItemRemoved(_)
        | Event::ItemMatched(_) => return None,
    })
}

/// What `request` asks for, with the year it came out if known: `Dune (2021)` or
/// `Dark Season 2 (2019)`.
fn title(request: &Request) -> String {
    let (title, date) = match request.movie.as_option() {
        Some(movie) => (movie.title.clone(), movie.release_date.as_deref()),
        None => (
            format!(
                "{} Season {}",
                request.season.show_title, request.season.season_number
            ),
            request.season.air_date.as_deref(),
        ),
    };
    // Dates are `YYYY-MM-DD`.
    match date.and_then(|date| date.get(..4)) {
        Some(year) => format!("{title} ({year})"),
        None => title,
    }
}

/// The Glossary's name for `request`'s state.
fn state(request: &Request) -> Option<&'static str> {
    Some(match request.state.as_known()? {
        RequestState::REQUEST_STATE_PENDING => "Pending",
        RequestState::REQUEST_STATE_APPROVED => "Approved",
        RequestState::REQUEST_STATE_DECLINED => "Declined",
        RequestState::REQUEST_STATE_PROCESSING => "Processing",
        RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE => "Partially Available",
        RequestState::REQUEST_STATE_AVAILABLE => "Available",
        RequestState::REQUEST_STATE_FAILED => "Failed",
        RequestState::REQUEST_STATE_UNSPECIFIED => return None,
    })
}

#[cfg(test)]
mod tests {
    use nama_pdk::{
        Notification,
        nama_proto::nama::v1::{
            Event, Request, RequestCancelled, RequestProgress, RequestState, RequestStateChanged,
            RequestedMovie, RequestedSeason, Requester, RequesterAdded, event,
        },
    };

    use super::message;

    fn requester(user_id: i64, name: &str) -> Requester {
        Requester {
            user_id,
            name: name.to_owned(),
            ..Requester::default()
        }
    }

    /// Dune, released in 2021, in `state`, asked for by Sam then Ada.
    fn dune(state: RequestState) -> Request {
        Request {
            movie: RequestedMovie {
                title: "Dune".to_owned(),
                release_date: Some("2021-09-15".to_owned()),
                ..RequestedMovie::default()
            }
            .into(),
            state: state.into(),
            requesters: vec![requester(1, "Sam"), requester(2, "Ada")],
            ..Request::default()
        }
    }

    fn notify(event: impl Into<Option<event::Event>>, request: Request) -> Option<String> {
        let event = Event {
            event: event.into(),
            ..Event::default()
        };
        message(&Notification {
            event: event.into(),
            request: request.into(),
            ..Notification::default()
        })
    }

    #[test]
    fn a_failed_request_says_why() {
        let request = Request {
            failure_reason: Some("Radarr didn't answer".to_owned()),
            ..dune(RequestState::REQUEST_STATE_FAILED)
        };

        let posted = notify(RequestStateChanged::default(), request);

        assert_eq!(
            posted.as_deref(),
            Some("Dune (2021) is now Failed. Reason: Radarr didn't answer")
        );
    }

    #[test]
    fn a_season_is_named_by_its_show_and_number() {
        let request = Request {
            movie: None.into(),
            season: RequestedSeason {
                show_title: "Dark".to_owned(),
                season_number: 2,
                air_date: Some("2019-06-21".to_owned()),
                ..RequestedSeason::default()
            }
            .into(),
            ..dune(RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE)
        };

        let posted = notify(RequestStateChanged::default(), request);

        assert_eq!(
            posted.as_deref(),
            Some("Dark Season 2 (2019) is now Partially Available.")
        );
    }

    #[test]
    fn a_requester_joining_is_named() {
        let added = RequesterAdded {
            user_id: 2,
            ..RequesterAdded::default()
        };

        let posted = notify(added, dune(RequestState::REQUEST_STATE_PENDING));

        assert_eq!(posted.as_deref(), Some("Ada also asked for Dune (2021)."));
    }

    #[test]
    fn a_message_ends_with_the_link_to_the_request_the_server_gives() {
        let notification = Notification {
            event: Event {
                event: Some(RequestStateChanged::default().into()),
                ..Event::default()
            }
            .into(),
            request: dune(RequestState::REQUEST_STATE_PENDING).into(),
            request_url: Some("https://nama.example.com/requests#request-7".to_owned()),
            ..Notification::default()
        };

        let posted = message(&notification);

        assert_eq!(
            posted.as_deref(),
            Some(
                "Dune (2021) is now Pending.\n\
                 https://nama.example.com/requests#request-7"
            )
        );
    }

    #[test]
    fn a_cancelled_request_is_posted_but_download_progress_is_not() {
        let request = dune(RequestState::REQUEST_STATE_PROCESSING);

        let cancelled = notify(RequestCancelled::default(), request.clone());
        let progress = notify(RequestProgress::default(), request);

        assert_eq!(
            cancelled.as_deref(),
            Some("The Request for Dune (2021) was cancelled.")
        );
        assert_eq!(progress, None);
    }
}
