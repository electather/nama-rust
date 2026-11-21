use nama_pdk::{
    Connector, Notification, Notifier, Recipient, RecipientGone, Response,
    extism_pdk::{
        Error,
        json::{self, Value, json},
    },
    nama_plugin_proto::nama::plugin::v1::{
        Event, Request, RequestCancelled, RequestState, RequestStateChanged, RequestedMovie,
        Requester, SettingKind, setting_value::Value as SettingValue,
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
fn telegram(status: u16, body: &str) -> Kernel {
    let body = body.to_owned();
    Kernel::new()
        .setting("bot_token", "123:abc")
        .setting("channel", "@requests")
        .setting("api_url", "http://telegram.test")
        .http(move |_request, _body| Ok(Response::new(status, body.clone())))
}

fn refusal(description: &str) -> String {
    json!({"ok": false, "description": description}).to_string()
}

/// The URL and JSON body of each request `kernel` was sent, in order.
fn sent(kernel: &Kernel) -> Vec<(&str, Value)> {
    kernel
        .sent()
        .iter()
        .map(|sent| {
            (
                sent.request.url.as_str(),
                json::from_slice(&sent.body).unwrap(),
            )
        })
        .collect()
}

/// The error `deliver` fails with, posting Dune turning Pending to the channel through `kernel`.
fn channel_post_error(kernel: &mut Kernel) -> Error {
    with(kernel, || {
        Telegram::deliver(dune_pending(), Recipient::Channel(Box::default()))
    })
    .unwrap_err()
}

#[test]
fn the_channel_is_posted_the_message_with_the_link_to_the_request_the_server_gives() {
    let mut kernel = telegram(200, include_str!("../fixtures/sendMessage.json"));
    let linked = Notification {
        request_url: Some("https://nama.example.com/requests/7".to_owned()),
        ..dune_pending()
    };

    with(&mut kernel, || {
        Telegram::deliver(linked, Recipient::Channel(Box::default()))
    })
    .unwrap();

    assert_eq!(
        sent(&kernel),
        [(
            "http://telegram.test/bot123:abc/sendMessage",
            json!({
                "chat_id": "@requests",
                "text": "Dune (2021) is now Pending.\nhttps://nama.example.com/requests/7",
            })
        )]
    );
}

#[test]
fn a_cancelled_request_the_server_gives_no_link_is_posted_without_one() {
    let mut kernel = telegram(200, include_str!("../fixtures/sendMessage.json"));
    let cancelled = Notification {
        event: Event {
            event: Some(RequestCancelled::default().into()),
            ..Event::default()
        }
        .into(),
        ..dune_pending()
    };

    with(&mut kernel, || {
        Telegram::deliver(cancelled, Recipient::Channel(Box::default()))
    })
    .unwrap();

    assert_eq!(
        sent(&kernel),
        [(
            "http://telegram.test/bot123:abc/sendMessage",
            json!({"chat_id": "@requests", "text": "The Request for Dune (2021) was cancelled."})
        )]
    );
}

#[test]
fn with_no_channel_set_nothing_is_sent() {
    let mut kernel = Kernel::new()
        .setting("bot_token", "123:abc")
        .setting("api_url", "http://telegram.test");

    with(&mut kernel, || {
        Telegram::deliver(dune_pending(), Recipient::Channel(Box::default()))
    })
    .unwrap();

    assert_eq!(kernel.sent(), []);
}

#[test]
fn a_refused_or_malformed_bot_token_fails_saying_telegram_refused_it() {
    // Telegram answers 401 to an unknown token, and 404 to one not shaped like a token.
    for (status, answer) in [
        (401, include_str!("../fixtures/unauthorized.json")),
        (404, include_str!("../fixtures/not-found.json")),
    ] {
        let error = channel_post_error(&mut telegram(status, answer));

        assert_eq!(error.to_string(), "Telegram refused the bot token");
    }
}

#[test]
fn an_unreachable_telegram_fails_saying_it_didnt_answer() {
    let mut kernel = telegram(200, "").http(|_request, _body| Err(Error::msg("refused")));

    let error = channel_post_error(&mut kernel);

    assert_eq!(error.to_string(), "Telegram didn't answer");
}

#[test]
fn an_answer_that_is_not_json_fails_as_making_no_sense() {
    let mut kernel = telegram(200, "<html><body>Bad Gateway</body></html>");

    let error = channel_post_error(&mut kernel);

    assert!(
        error
            .to_string()
            .starts_with("Telegram's answer made no sense: "),
        "{error}"
    );
    assert!(!error.to_string().contains("123:abc"), "{error}");
}

#[test]
fn starting_a_connection_answers_the_bots_deep_link_with_the_code() {
    let mut kernel = telegram(200, include_str!("../fixtures/getMe.json"));

    let started = with(&mut kernel, || {
        Telegram::start_connection("CODE".to_owned())
    })
    .unwrap();

    assert_eq!(started.url, "https://t.me/nama_test_bot?start=CODE");
    assert_eq!(started.pending, "CODE");
    assert_eq!(
        sent(&kernel),
        [("http://telegram.test/bot123:abc/getMe", json!({}))]
    );
}

#[test]
fn polling_finds_the_chat_that_sent_start_with_the_code() {
    let mut kernel = telegram(200, include_str!("../fixtures/getUpdates.json"));

    let connected = with(&mut kernel, || Telegram::poll_connection("CODE".to_owned())).unwrap();

    assert_eq!(connected.as_deref(), Some("987654321"));
    assert_eq!(
        sent(&kernel),
        [(
            "http://telegram.test/bot123:abc/getUpdates",
            json!({"offset": -100})
        )]
    );
}

#[test]
fn polling_ignores_a_start_with_another_code() {
    let mut kernel = telegram(200, include_str!("../fixtures/getUpdates.json"));

    let connected = with(&mut kernel, || {
        Telegram::poll_connection("someone-elses-code".to_owned())
    })
    .unwrap();

    assert_eq!(connected, None);
}

#[test]
fn a_users_chat_refusing_the_bot_means_the_recipient_is_gone() {
    let mut kernel = telegram(403, include_str!("../fixtures/forbidden.json"));

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
    let posted: Value = json::from_slice(&sent.body).unwrap();
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
fn telegram_limiting_the_bot_says_the_seconds_its_body_gives() {
    let mut kernel = telegram(
        429,
        r#"{"ok":false,"error_code":429,"description":"Too Many Requests: retry after 5","parameters":{"retry_after":5}}"#,
    );

    assert_eq!(
        channel_post_error(&mut kernel).to_string(),
        "Telegram is limiting this Server's requests; try again in 5 s"
    );
}

#[test]
fn telegram_limiting_the_bot_without_parameters_says_a_few_seconds() {
    let mut kernel = telegram(429, &refusal("Too Many Requests: retry after 5"));

    assert_eq!(
        channel_post_error(&mut kernel).to_string(),
        "Telegram is limiting this Server's requests; try again in a few seconds"
    );
}

#[test]
fn the_channel_refusing_the_bot_is_a_failure_not_a_recipient_gone() {
    let mut kernel = telegram(
        403,
        &refusal("Forbidden: bot is not a member of the channel"),
    );

    let error = channel_post_error(&mut kernel);

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
