//! A minimal Installed Plugin to copy: it serves one Extension Point, the Notifier, by posting
//! each Request event's name and title as JSON to the webhook its settings name. The Server
//! doesn't bundle it; CI builds it for wasm32 so it keeps up with `nama-pdk`.

use nama_pdk::{
    Manifest, Notification, OutsideService, Recipient, SettingField, SettingKind,
    extism_pdk::{Error, HttpRequest, Json, json::json},
    nama_plugin_proto::nama::plugin::v1::{Request, event::Event, request::MovieOrSeason},
    services, setting_field,
};

/// The setting holding the URL to post to.
const WEBHOOK_URL: &str = "webhook_url";

/// The service the Plugin posts to, named as the subject of its errors, such as "the webhook
/// didn't answer". Name yours; add `.credential("API key")` if it takes one, so a 401 names it,
/// and `.reader(...)` to quote the error message its refusals carry.
const WEBHOOK: OutsideService = OutsideService::new("the webhook");

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        // Change these to your Plugin's name and what it does.
        name: "Template".to_owned(),
        description: "Posts Request events to a webhook.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        // Change this to the point your Plugin serves, one of `nama_pdk::services`, and export
        // it with its macro, as `nama_pdk::notifier!` does below.
        points: vec![nama_pdk::point(services::NOTIFIER)],
        // Set `required`, a `default_value` from `nama_pdk::setting_default`, or an Integer's
        // bounds with struct update syntax on `setting_field`.
        settings: vec![SettingField {
            required: true,
            // A URL setting's host is one the Plugin may reach, so `allowed_hosts` stays empty.
            ..setting_field(
                WEBHOOK_URL,
                "Webhook URL",
                "Where each Request event is posted as JSON.",
                SettingKind::SETTING_KIND_URL,
            )
        }],
        // Change the id, which the Server keeps the Plugin's settings under.
        ..nama_pdk::manifest("template", env!("CARGO_PKG_VERSION"))
    }
}

/// Posts each Request event for the channel to the webhook.
struct Template;

impl nama_pdk::Notifier for Template {
    fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
        // Without a Connector, the Server sends nothing for a User's Plugin Connection.
        let Recipient::Channel(_) = recipient else {
            return Ok(());
        };
        let Some((event, title)) = what_happened(&notification) else {
            return Ok(());
        };
        // Read a setting the Plugin can't work without this way, so a blank one fails naming
        // its key before anything is sent; read an optional one with `nama_pdk::setting`.
        let url = nama_pdk::required_setting(WEBHOOK_URL)?;
        let request = HttpRequest::new(url)
            .with_method("POST")
            .with_header("Content-Type", "application/json");
        let body = Json(json!({ "event": event, "title": title }));
        // Match the statuses that mean something to your Plugin on the response first, and hand
        // the rest to `accepted`, which words every refusal the way the other Plugins do.
        WEBHOOK.accepted(WEBHOOK.send(&request, Some(body))?)?;
        Ok(())
    }
}

nama_pdk::notifier!(Template);

/// The event's name and its Request's title, or `None` for an event that isn't about a Request.
fn what_happened(notification: &Notification) -> Option<(&'static str, String)> {
    let event = match notification.event.as_option()?.event.as_ref()? {
        Event::RequestCreated(_) => "request_created",
        Event::RequesterAdded(_) => "requester_added",
        Event::RequestStateChanged(_) => "request_state_changed",
        Event::RequestCancelled(_) => "request_cancelled",
        Event::RequesterRemoved(_) => "requester_removed",
        Event::RequestWithdrawn(_) => "request_withdrawn",
        Event::ScanStarted(_)
        | Event::ScanProgress(_)
        | Event::ScanFinished(_)
        | Event::LibraryItemAdded(_)
        | Event::LibraryItemRemoved(_)
        | Event::ItemMatched(_)
        | Event::MatchingFinished(_)
        | Event::ProbingFinished(_)
        | Event::PlaybackStarted(_)
        | Event::PlaybackEnded(_)
        | Event::PlaybackChanged(_)
        | Event::ServerLinked(_)
        | Event::ServerUnlinked(_)
        | Event::UserLinked(_)
        | Event::UserUnlinked(_) => return None,
    };
    Some((event, title(notification.request.as_option()?)))
}

/// What `request` asks for, such as `Dune` or `Dark Season 2`.
fn title(request: &Request) -> String {
    match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => movie.title.clone(),
        Some(MovieOrSeason::Season(season)) => {
            format!("{} Season {}", season.show_title, season.season_number)
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use nama_pdk::{
        Notification, Notifier, Recipient, Response,
        extism_pdk::json::{self, Value, json},
        nama_plugin_proto::nama::plugin::v1::{
            Event, Request, RequestCreated, RequestedMovie, SettingKind,
        },
        services,
        testing::{Kernel, with},
    };

    use super::{Template, WEBHOOK_URL, manifest};

    // Call your Extension Point method through `nama_pdk::testing`: the Kernel it installs holds
    // your settings, answers your requests, and keeps what you sent.
    #[test]
    fn a_request_event_posts_its_name_and_title_to_the_webhook() {
        let mut kernel = Kernel::new()
            .setting(WEBHOOK_URL, "http://hooks.test/nama")
            .http(|_request, _body| Ok(Response::new(204, "")));
        let created = Notification {
            event: Event {
                event: Some(RequestCreated::default().into()),
                ..Event::default()
            }
            .into(),
            request: Request {
                movie_or_season: RequestedMovie {
                    title: "Dune".to_owned(),
                    ..RequestedMovie::default()
                }
                .into(),
                ..Request::default()
            }
            .into(),
            ..Notification::default()
        };

        with(&mut kernel, || {
            Template::deliver(created, Recipient::Channel(Box::default()))
        })
        .unwrap();

        let [sent] = kernel.sent() else {
            panic!(
                "the webhook was sent {} posts, not one",
                kernel.sent().len()
            );
        };
        assert_eq!(sent.request.url, "http://hooks.test/nama");
        let posted: Value = json::from_slice(&sent.body).unwrap();
        assert_eq!(posted, json!({"event": "request_created", "title": "Dune"}));
    }

    #[test]
    fn manifest_names_the_notifier_point_and_a_required_webhook_url_setting() {
        let manifest = manifest();

        let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
        assert_eq!(points, [services::NOTIFIER]);
        let [setting] = manifest.settings.as_slice() else {
            panic!("{:?}", manifest.settings);
        };
        assert_eq!(setting.key, WEBHOOK_URL);
        assert!(setting.required);
        assert_eq!(setting.kind, SettingKind::SETTING_KIND_URL);
    }
}
