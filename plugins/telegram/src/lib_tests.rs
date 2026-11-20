use nama_pdk::{
    Notification, Notifier, Recipient, RecipientGone, Response,
    extism_pdk::json::{Value, json},
    nama_plugin_proto::nama::plugin::v1::{
        Event, Request, RequestState, RequestStateChanged, RequestedMovie, Requester, SettingKind,
        setting_value::Value as SettingValue,
    },
    testing::{Kernel, with},
};

use super::{exports::Telegram, manifest, message};

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
        movie_or_season: RequestedMovie {
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

/// Dune's Request turning Pending, as the Server tells it.
fn dune_pending() -> Notification {
    Notification {
        event: Event {
            event: Some(RequestStateChanged::default().into()),
            ..Event::default()
        }
        .into(),
        request: dune(RequestState::REQUEST_STATE_PENDING).into(),
        ..Notification::default()
    }
}

/// Telegram's settings, posting to the channel `@requests`, answering every call with `status`
/// and `body`.
fn telegram(status: u16, body: &Value) -> Kernel {
    let body = body.to_string();
    Kernel::new()
        .setting("bot_token", "123:abc")
        .setting("channel", "@requests")
        .setting("api_url", "http://telegram.test")
        .http(move |_request, _body| Ok(Response::new(status, body.clone())))
}

fn refusal(description: &str) -> Value {
    json!({"ok": false, "description": description})
}

#[test]
fn a_users_chat_refusing_the_bot_means_the_recipient_is_gone() {
    let mut kernel = telegram(403, &refusal("Forbidden: bot was blocked by the user"));

    let error = with(&mut kernel, || {
        Telegram::deliver(dune_pending(), Recipient::Connection("42".to_owned()))
    })
    .unwrap_err();

    assert!(error.is::<RecipientGone>(), "{error}");
    let [sent] = kernel.sent() else {
        panic!(
            "Telegram was sent {} requests, not one",
            kernel.sent().len()
        );
    };
    assert_eq!(
        sent.request.url,
        "http://telegram.test/bot123:abc/sendMessage"
    );
    let posted: Value = nama_pdk::extism_pdk::json::from_slice(&sent.body).unwrap();
    assert_eq!(posted["chat_id"], "42");
}

#[test]
fn a_users_chat_failing_otherwise_says_telegrams_words_and_the_recipient_stays() {
    let mut kernel = telegram(400, &refusal("Bad Request: chat not found"));

    let error = with(&mut kernel, || {
        Telegram::deliver(dune_pending(), Recipient::Connection("42".to_owned()))
    })
    .unwrap_err();

    assert!(!error.is::<RecipientGone>());
    assert_eq!(
        error.to_string(),
        "Telegram said: Bad Request: chat not found"
    );
}

#[test]
fn the_channel_refusing_the_bot_is_a_failure_not_a_recipient_gone() {
    let mut kernel = telegram(
        403,
        &refusal("Forbidden: bot is not a member of the channel"),
    );

    let error = with(&mut kernel, || {
        Telegram::deliver(dune_pending(), Recipient::Channel(Box::default()))
    })
    .unwrap_err();

    assert!(!error.is::<RecipientGone>());
    assert_eq!(
        error.to_string(),
        "Telegram said: Forbidden: bot is not a member of the channel"
    );
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
        request_url: Some("https://nama.example.com/requests/7".to_owned()),
        ..Notification::default()
    };

    let posted = message(&notification);

    assert_eq!(
        posted.as_deref(),
        Some(
            "Dune (2021) is now Pending.\n\
             https://nama.example.com/requests/7"
        )
    );
}

#[test]
fn manifest_declares_a_bot_token_a_channel_and_the_bot_api_url_defaulting_to_telegram() {
    let settings = manifest().settings;
    let settings: Vec<_> = settings
        .iter()
        .map(|field| {
            let default = field.default_value.as_option();
            (
                field.key.as_str(),
                field.kind.as_known(),
                field.required,
                default.and_then(|default| default.value.clone()),
            )
        })
        .collect();

    assert_eq!(
        settings,
        [
            (
                "bot_token",
                Some(SettingKind::SETTING_KIND_SECRET),
                true,
                None
            ),
            ("channel", Some(SettingKind::SETTING_KIND_TEXT), false, None),
            (
                "api_url",
                Some(SettingKind::SETTING_KIND_URL),
                false,
                Some(SettingValue::TextValue(
                    "https://api.telegram.org".to_owned()
                )),
            ),
        ]
    );
}
