use nama_pdk::{
    Notification, Notifier, Recipient, RecipientGone, Response,
    extism_pdk::json::{self, Value as Json, json},
    nama_plugin_proto::nama::plugin::v1::{
        Event, Request, RequestState, RequestStateChanged, RequestedMovie, ServerLinked,
        SettingKind, UserRecipient, setting_value::Value,
    },
    testing::{Kernel, with},
};

use super::{exports::Discord, manifest, webhook};

/// Dune, released in 2021, turning Approved, with the link to its Request.
fn dune_approved() -> Notification {
    Notification {
        event: Event {
            event: Some(RequestStateChanged::default().into()),
            ..Event::default()
        }
        .into(),
        request: Request {
            movie_or_season: RequestedMovie {
                title: "Dune".to_owned(),
                release_date: Some("2021-09-15".to_owned()),
                ..RequestedMovie::default()
            }
            .into(),
            state: RequestState::REQUEST_STATE_APPROVED.into(),
            ..Request::default()
        }
        .into(),
        request_url: Some("https://nama.example.com/requests/7".to_owned()),
        ..Notification::default()
    }
}

/// The User whose Personal setting webhook is `url`.
fn user(url: &str) -> Recipient {
    Recipient::User(Box::new(UserRecipient {
        personal_settings: std::iter::once(("webhook".to_owned(), url.to_owned())).collect(),
        ..UserRecipient::default()
    }))
}

/// Discord's settings at `http://discord.test`, answering every post with `status` and `body`.
fn discord(status: u16, body: &Json) -> Kernel {
    let body = body.to_string();
    Kernel::new()
        .setting("api_url", "http://discord.test")
        .http(move |_request, _body| Ok(Response::new(status, body.clone())))
}

#[test]
fn a_message_posts_to_the_users_own_webhook_on_the_discord_url_setting() {
    let mut kernel = discord(204, &json!(null));

    with(&mut kernel, || {
        Discord::deliver(
            dune_approved(),
            user("https://discordapp.com/api/webhooks/1/ada-token"),
        )
    })
    .unwrap();

    let [sent] = kernel.sent() else {
        panic!("Discord was sent {} requests, not one", kernel.sent().len());
    };
    assert_eq!(
        sent.request.url,
        "http://discord.test/api/webhooks/1/ada-token"
    );
    assert_eq!(sent.request.method.as_deref(), Some("POST"));
    let posted: Json = json::from_slice(&sent.body).unwrap();
    assert_eq!(
        posted,
        json!({"content": "Dune (2021) is now Approved.\nhttps://nama.example.com/requests/7"})
    );
}

/// The Server Linked to the Hub, as the Kernel tells an Admin.
fn server_linked() -> Notification {
    Notification {
        event: Event {
            event: Some(ServerLinked::default().into()),
            ..Event::default()
        }
        .into(),
        ..Notification::default()
    }
}

#[test]
fn an_admin_is_told_of_linking_unless_they_turned_announcing_it_off() {
    let webhook = "https://discord.com/api/webhooks/1/ada-token";
    let mut on = discord(204, &json!(null)).setting("announce_linking", "true");
    let mut off = discord(204, &json!(null)).setting("announce_linking", "false");

    with(&mut on, || Discord::deliver(server_linked(), user(webhook))).unwrap();
    with(&mut off, || {
        Discord::deliver(server_linked(), user(webhook))
    })
    .unwrap();

    let [sent] = on.sent() else {
        panic!("Discord was sent {} requests, not one", on.sent().len());
    };
    let posted: Json = json::from_slice(&sent.body).unwrap();
    assert_eq!(
        posted,
        json!({"content": "The Server was Linked to the Hub."})
    );
    assert_eq!(off.sent(), []);
}

#[test]
fn a_webhook_discord_no_longer_knows_means_the_recipient_is_gone() {
    let mut kernel = discord(404, &json!({"message": "Unknown Webhook", "code": 10015}));

    let error = with(&mut kernel, || {
        Discord::deliver(
            dune_approved(),
            user("https://discord.com/api/webhooks/1/gone-token"),
        )
    })
    .unwrap_err();

    assert!(error.is::<RecipientGone>(), "{error}");
}

#[test]
fn another_refusal_quotes_discords_own_message() {
    let mut kernel = discord(
        401,
        &json!({"message": "Invalid Webhook Token", "code": 50027}),
    );

    let error = with(&mut kernel, || {
        Discord::deliver(
            dune_approved(),
            user("https://discord.com/api/webhooks/1/ada-token"),
        )
    })
    .unwrap_err();

    assert!(!error.is::<RecipientGone>());
    assert_eq!(error.to_string(), "Discord said: Invalid Webhook Token");
}

#[test]
fn an_invalid_webhook_is_refused_naming_the_setting_never_its_value_and_nothing_is_sent() {
    let mut kernel = discord(204, &json!(null));

    let error = with(&mut kernel, || {
        Discord::deliver(
            dune_approved(),
            user("https://discord.com/api/webhooks/123/secret_token/github"),
        )
    })
    .unwrap_err()
    .to_string();

    assert_eq!(
        error,
        "the Personal setting webhook must be a Discord webhook URL, with /api/webhooks/ in it"
    );
    assert_eq!(kernel.sent(), []);
}

#[test]
fn the_id_and_token_after_api_webhooks_are_taken_whatever_the_host() {
    for (given, taken) in [
        (
            "https://discord.com/api/webhooks/123/tok_EN-9",
            "123/tok_EN-9",
        ),
        ("https://discordapp.com/api/webhooks/123/tok/", "123/tok"),
        (
            "https://ptb.discord.com/api/webhooks/123/tok?wait=true",
            "123/tok",
        ),
        ("http://127.0.0.1:9/api/webhooks/456/other", "456/other"),
    ] {
        assert_eq!(webhook(Some(given)).ok().as_deref(), Some(taken), "{given}");
    }
}

#[test]
fn a_webhook_without_an_id_and_token_is_refused_naming_the_setting_not_its_value() {
    for refused in [
        "",
        "https://discord.com/channels/123/456",
        "https://discord.com/api/webhooks/123",
        "https://discord.com/api/webhooks/123/",
        "https://discord.com/api/webhooks/abc/secret_token",
        "https://discord.com/api/webhooks/123/secret_token/github",
        "https://discord.com/api/webhooks/123/../secret_token",
    ] {
        let error = webhook(Some(refused)).unwrap_err().to_string();
        assert!(
            error.contains("Personal setting webhook"),
            "{refused}: {error}"
        );
        assert!(!error.contains("secret_token"), "{refused}: {error}");
    }
}

#[test]
fn a_missing_webhook_is_refused_naming_the_setting() {
    let error = webhook(None).unwrap_err().to_string();

    assert!(error.contains("Personal setting webhook"), "{error}");
}

#[test]
fn manifest_declares_the_discord_url_defaulting_to_discord_com_and_announcing_linking() {
    let settings = manifest().settings;
    let settings: Vec<_> = settings
        .iter()
        .map(|field| {
            let default = field.default_value.as_option();
            (
                field.key.as_str(),
                field.kind.as_known(),
                default.and_then(|default| default.value.clone()),
            )
        })
        .collect();

    assert_eq!(
        settings,
        [
            (
                "api_url",
                Some(SettingKind::SETTING_KIND_URL),
                Some(Value::TextValue("https://discord.com".to_owned())),
            ),
            (
                "announce_linking",
                Some(SettingKind::SETTING_KIND_BOOL),
                Some(Value::BoolValue(true)),
            ),
        ]
    );
}
