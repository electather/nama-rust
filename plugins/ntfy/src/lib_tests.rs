use nama_pdk::{
    Notification, Notifier, Recipient, RecipientGone, Response,
    nama_plugin_proto::nama::plugin::v1::{
        Event, PersonalSettingsCheck, Request, RequestState, RequestStateChanged, RequestedMovie,
        SettingKind, UserRecipient, setting_value::Value,
    },
    testing::{Kernel, Sent, with},
};

use super::{TOPIC, check_personal_settings, exports::Ntfy, manifest, topic};

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

fn user(personal_settings: &[(&str, &str)]) -> Recipient {
    Recipient::User(Box::new(UserRecipient {
        personal_settings: personal_settings
            .iter()
            .map(|&(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        ..UserRecipient::default()
    }))
}

/// ntfy's settings at `http://ntfy.test`, answering every message with `status` and `body`.
fn ntfy(status: u16, body: &'static str) -> Kernel {
    Kernel::new()
        .setting("server_url", "http://ntfy.test")
        .http(move |_request, _body| Ok(Response::new(status, body)))
}

#[test]
fn a_message_goes_to_each_users_own_topic_with_their_own_access_token_if_set() {
    let mut kernel = ntfy(200, "{}");

    with(&mut kernel, || {
        Ntfy::deliver(
            dune_approved(),
            user(&[("topic", "ada-topic"), ("access_token", "tk_ada")]),
        )?;
        Ntfy::deliver(dune_approved(), user(&[("topic", "ben-topic")]))
    })
    .unwrap();

    let [ada, ben] = kernel.sent() else {
        panic!("ntfy was sent {} messages, not two", kernel.sent().len());
    };
    assert_eq!(ada.request.url, "http://ntfy.test/ada-topic");
    assert_eq!(ben.request.url, "http://ntfy.test/ben-topic");
    for sent in [ada, ben] {
        let header = |name| sent.request.headers.get(name).map(String::as_str);
        assert_eq!(sent.request.method.as_deref(), Some("POST"));
        assert_eq!(header("Title"), Some("Nama"));
        assert_eq!(header("Click"), Some("https://nama.example.com/requests/7"));
        assert_eq!(sent.body, b"Dune (2021) is now Approved.");
    }
    let authorization = |sent: &Sent| sent.request.headers.get("Authorization").cloned();
    assert_eq!(authorization(ada).as_deref(), Some("Bearer tk_ada"));
    assert_eq!(authorization(ben), None);
}

#[test]
fn a_topic_ntfy_refuses_fails_in_ntfys_words() {
    let mut kernel = ntfy(403, r#"{"code":40301,"http":403,"error":"forbidden"}"#);

    let error = with(&mut kernel, || {
        Ntfy::deliver(dune_approved(), user(&[("topic", "ada-topic")]))
    })
    .unwrap_err();

    assert!(!error.is::<RecipientGone>());
    assert_eq!(error.to_string(), "ntfy said: forbidden");
}

#[test]
fn a_refused_access_token_is_named() {
    let mut kernel = ntfy(401, r#"{"code":40101,"http":401,"error":"unauthorized"}"#);

    let error = with(&mut kernel, || {
        Ntfy::deliver(
            dune_approved(),
            user(&[("topic", "ada-topic"), ("access_token", "tk_ada")]),
        )
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "ntfy refused the access token");
}

#[test]
fn a_topic_of_letters_digits_underscores_and_dashes_up_to_64_long_is_taken() {
    let longest = "a".repeat(64);

    for taken in ["a", "nama_ada-7Xq", longest.as_str()] {
        assert_eq!(topic(Some(taken)).ok(), Some(taken));
    }
}

#[test]
fn an_empty_too_long_or_oddly_lettered_topic_is_refused_naming_the_setting() {
    let too_long = "a".repeat(65);

    for refused in [
        "",
        too_long.as_str(),
        "with space",
        "a/b",
        "ünïcode",
        "dot.ted",
    ] {
        let error = topic(Some(refused)).map(drop).unwrap_err().to_string();
        assert!(
            error.contains("Personal setting topic"),
            "{refused}: {error}"
        );
    }
}

fn asked(topic: &str) -> PersonalSettingsCheck {
    PersonalSettingsCheck {
        personal_settings: std::iter::once((TOPIC.to_owned(), topic.to_owned())).collect(),
        ..PersonalSettingsCheck::default()
    }
}

#[test]
fn the_personal_settings_check_passes_a_good_topic_and_refuses_a_bad_one_in_its_words() {
    let passed = check_personal_settings(&asked("nama_ada-7Xq"));
    let refused = check_personal_settings(&asked("bad topic/../!"));

    assert!(passed.is_ok(), "{passed:?}");
    assert_eq!(
        refused.unwrap_err().to_string(),
        "the Personal setting topic must be 1 to 64 letters, digits, _ or -"
    );
}

#[test]
fn a_missing_topic_is_refused_naming_the_setting() {
    let error = topic(None).map(drop).unwrap_err().to_string();

    assert!(error.contains("Personal setting topic"), "{error}");
}

#[test]
fn manifest_declares_the_ntfy_server_defaulting_to_ntfy_sh() {
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
        [(
            "server_url",
            Some(SettingKind::SETTING_KIND_URL),
            Some(Value::TextValue("https://ntfy.sh".to_owned())),
        )]
    );
}
