use nama_pdk::{
    Notification,
    nama_plugin_proto::nama::plugin::v1::{
        Event, Request, RequestState, RequestStateChanged, RequestedMovie, Requester, SettingKind,
        setting_value::Value,
    },
};

use super::{manifest, message};

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
                Some(Value::TextValue("https://api.telegram.org".to_owned())),
            ),
        ]
    );
}
