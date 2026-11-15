//! A notification's wording, shared by the Notifier Plugins so each says the same.

use crate::{
    Notification,
    nama_plugin_proto::nama::plugin::v1::{
        Request, RequestState, event::Event, request::MovieOrSeason,
    },
};

/// What happened, or `None` for what a Notifier doesn't tell: an event that isn't about a Request,
/// or a state this nama-pdk doesn't know.
pub fn what_happened(notification: &Notification) -> Option<String> {
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
            let quality = approved_at(request);
            format!("{title} is now {}{quality}.{reason}", state(request)?)
        }
        Event::RequestCancelled(cancelled) => match &cancelled.deleted_by {
            Some(by) => format!("{by} deleted {title}, so its Request was cancelled."),
            None => format!("The Request for {title} was cancelled."),
        },
        Event::RequesterRemoved(removed) => format!(
            "{} withdrew from the Request for {title}.",
            name(removed.user_id)
        ),
        Event::RequestWithdrawn(_) => format!("The Request for {title} was withdrawn."),
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
        | Event::PlaybackChanged(_) => return None,
    })
}

/// What `request` asks for, with the year it came out if known: `Dune (2021)` or
/// `Dark Season 2 (2019)`.
fn title(request: &Request) -> String {
    let (title, date) = match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => (movie.title.clone(), movie.release_date.as_deref()),
        Some(MovieOrSeason::Season(season)) => (
            format!("{} Season {}", season.show_title, season.season_number),
            season.air_date.as_deref(),
        ),
        // The Server sets one on every Request it sends.
        None => (String::new(), None),
    };
    // Dates are `YYYY-MM-DD`.
    match date.and_then(|date| date.get(..4)) {
        Some(year) => format!("{title} ({year})"),
        None => title,
    }
}

/// What an Approved `request` is downloaded at, and who last changed that from what, such as
/// ` at HD, changed from 4K by Sam`, or nothing if it has no Quality or isn't Approved.
fn approved_at(request: &Request) -> String {
    let Some(quality) = request.quality.as_option() else {
        return String::new();
    };
    if request.state != RequestState::REQUEST_STATE_APPROVED {
        return String::new();
    }
    let Some(change) = request.quality_change.as_option() else {
        return format!(" at {}", quality.name);
    };
    let from = change
        .previous_name
        .as_ref()
        .map_or_else(String::new, |previous| format!(" from {previous}"));
    let by = change.changed_by.as_deref().unwrap_or("a Request manager");
    format!(" at {}, changed{from} by {by}", quality.name)
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
#[path = "notification_text_tests.rs"]
mod tests;
