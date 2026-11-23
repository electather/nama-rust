//! The conversions `LinkedServerService.PushTrackPreferences` and `PullTrackPreferences` use.

use connectrpc::ConnectError;
use nama_hub_proto::nama::hub::v1::{self as proto, RefusalReason, TitleKind};

use super::refuse;
use crate::sync::{
    Like, MAX_CHANGED_AT_MS, MAX_CHANGES, MAX_ID_LENGTH, MAX_LANGUAGE_LENGTH, MAX_TITLE_LENGTH,
    PreferenceFor, TrackPreferenceChange,
};

/// `changes` as `sync` takes them, or the refusal of the first limit they break.
pub(super) fn changes_from_proto(
    changes: Vec<proto::TrackPreferenceChange>,
) -> Result<Vec<TrackPreferenceChange>, ConnectError> {
    if changes.len() > MAX_CHANGES {
        return Err(invalid(
            "changes",
            format!("a push holds up to {MAX_CHANGES} changes"),
        ));
    }
    (changes.into_iter().enumerate())
        .map(|(nth, change)| change_from_proto(nth, change))
        .collect()
}

/// The `nth` change of a push, as `sync` takes it.
fn change_from_proto(
    nth: usize,
    change: proto::TrackPreferenceChange,
) -> Result<TrackPreferenceChange, ConnectError> {
    let field = |name: &str| format!("changes[{nth}].{name}");
    let title_id = change.title_id;
    let id_fits = !title_id.is_empty() && title_id.chars().count() <= MAX_ID_LENGTH;
    let kept_for = match change.title_kind.as_known() {
        Some(TitleKind::TITLE_KIND_UNSPECIFIED) if title_id.is_empty() => PreferenceFor::User,
        Some(TitleKind::TITLE_KIND_MOVIE) if id_fits => PreferenceFor::Movie(title_id),
        Some(TitleKind::TITLE_KIND_SHOW) if id_fits => PreferenceFor::Show(title_id),
        _ => {
            return Err(invalid(
                &field("title_id"),
                format!(
                    "a Movie's or Show's Preference names its External ID of 1 to \
                     {MAX_ID_LENGTH} characters, and the User's own none"
                ),
            ));
        }
    };
    if change.language.chars().count() > MAX_LANGUAGE_LENGTH {
        return Err(invalid(&field("language"), "the language is too long"));
    }
    if change.title.chars().count() > MAX_TITLE_LENGTH {
        return Err(invalid(&field("title"), "the title is too long"));
    }
    if !(0..=MAX_CHANGED_AT_MS).contains(&change.changed_at_ms) {
        return Err(invalid(
            &field("changed_at_ms"),
            "a change is dated from 1970 to the end of 9999",
        ));
    }
    Ok(TrackPreferenceChange {
        user_id: change.user_id,
        audio: change.audio,
        kept_for,
        like: (!change.off).then_some(Like {
            language: change.language,
            forced: change.forced,
            sdh: change.sdh,
            title: change.title,
        }),
        foreign_audio_only: change.foreign_audio_only,
        changed_at_ms: change.changed_at_ms,
    })
}

/// `change`, as a Linked Server takes it.
pub(super) fn change_to_proto(change: TrackPreferenceChange) -> proto::TrackPreferenceChange {
    let (title_kind, title_id) = match change.kept_for {
        PreferenceFor::User => (TitleKind::TITLE_KIND_UNSPECIFIED, String::new()),
        PreferenceFor::Movie(id) => (TitleKind::TITLE_KIND_MOVIE, id),
        PreferenceFor::Show(id) => (TitleKind::TITLE_KIND_SHOW, id),
    };
    let off = change.like.is_none();
    let like = change.like.unwrap_or_default();
    proto::TrackPreferenceChange {
        user_id: change.user_id,
        audio: change.audio,
        title_kind: title_kind.into(),
        title_id,
        off,
        language: like.language,
        forced: like.forced,
        sdh: like.sdh,
        title: like.title,
        foreign_audio_only: change.foreign_audio_only,
        changed_at_ms: change.changed_at_ms,
        ..Default::default()
    }
}

/// Refuses a push for a change at `field` that breaks a limit, which `message` names.
fn invalid(field: &str, message: impl Into<String>) -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_TRACK_PREFERENCE_INVALID,
        message,
    )
    .field(field)
    .param("maxChanges", &MAX_CHANGES)
    .param("maxIdLength", &MAX_ID_LENGTH)
    .param("maxLanguageLength", &MAX_LANGUAGE_LENGTH)
    .param("maxTitleLength", &MAX_TITLE_LENGTH)
    .into()
}
