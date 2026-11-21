//! The access layer: who may call each RPC or fetch each path, checked before any handler runs.

use axum::{extract::Request, http::StatusCode, middleware::Next, response::IntoResponse};
use buffa::Enumeration;
use connectrpc::ConnectError;
use nama_proto::nama::v1::{self as proto, Permission, RefusalReason};

use super::{refuse, sign_in_required};
use crate::{db::now, diagnostics, users::Caller};

/// Who may call an RPC or fetch media.
#[derive(Clone, Copy)]
enum Access {
    Anyone,
    SignedIn,
    /// A signed-in User whose Permissions pass a check for this one.
    Needs(Permission),
}

/// Managing Libraries and their Matches.
const MANAGE_LIBRARIES: Access = Access::Needs(Permission::ManageLibraries);

/// Managing Users and their Devices.
const MANAGE_USERS: Access = Access::Needs(Permission::ManageUsers);

/// What only an Admin may do.
const ADMIN: Access = Access::Needs(Permission::Admin);

/// Managing the Server's settings, such as its Plugins.
const MANAGE_SETTINGS: Access = Access::Needs(Permission::ManageSettings);

/// Approving, declining and cancelling every User's Requests.
const MANAGE_REQUESTS: Access = Access::Needs(Permission::ManageRequests);

/// Asking for Movies.
const REQUEST_MOVIE: Access = Access::Needs(Permission::RequestMovie);

/// Asking for Seasons of Shows.
const REQUEST_TV: Access = Access::Needs(Permission::RequestTv);

/// Deleting media from the Libraries' folders.
const DELETE_MEDIA: Access = Access::Needs(Permission::DeleteMedia);

/// Searching the subtitle sources and downloading their subtitles.
const DOWNLOAD_SUBTITLES: Access = Access::Needs(Permission::DownloadSubtitles);

/// Who may call each RPC, by path; one missing here is refused to everyone (see AGENTS.md, "New
/// RPCs start closed"). Media and uploads have their own entries, in [`PREFIXES`].
// One row per RPC reads as a table; rustfmt would split most rows over four lines.
#[rustfmt::skip]
const ACCESS: &[(&str, Access)] = &[
    (proto::SERVER_SERVICE_GET_SERVER_INFO_SPEC.procedure,        Access::Anyone),
    (proto::SERVER_SERVICE_GET_SERVER_SETTINGS_SPEC.procedure,    MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_UPDATE_SERVER_SETTINGS_SPEC.procedure, MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_GET_PLAYBACK_STATUS_SPEC.procedure,    ADMIN),
    (proto::SERVER_SERVICE_GET_PLAYBACK_SETTINGS_SPEC.procedure,  ADMIN),
    (proto::SERVER_SERVICE_UPDATE_PLAYBACK_SETTINGS_SPEC.procedure, ADMIN),
    (proto::SERVER_SERVICE_GET_IMAGE_SETTINGS_SPEC.procedure,     MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_UPDATE_IMAGE_SETTINGS_SPEC.procedure,  MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_SWITCH_IMAGE_STORE_SPEC.procedure,     MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_GET_DEFAULT_HOME_LAYOUT_SPEC.procedure, MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_SET_DEFAULT_HOME_LAYOUT_SPEC.procedure, MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_LIST_PLAYS_SPEC.procedure,             ADMIN),
    (proto::SERVER_SERVICE_STOP_PLAY_SPEC.procedure,              ADMIN),
    (proto::SIGN_IN_SERVICE_SETUP_SPEC.procedure,                 Access::Anyone),
    (proto::SIGN_IN_SERVICE_SIGN_IN_SPEC.procedure,               Access::Anyone),
    (proto::SIGN_IN_SERVICE_SIGN_OUT_SPEC.procedure,              Access::SignedIn),
    (proto::SIGN_IN_SERVICE_WHO_AM_I_SPEC.procedure,              Access::SignedIn),
    (proto::SIGN_IN_SERVICE_REAUTHENTICATE_SPEC.procedure,        Access::SignedIn),
    (proto::SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC.procedure,    Access::Anyone),
    (proto::SIGN_IN_SERVICE_LOOK_UP_SIGN_IN_CODE_SPEC.procedure,  Access::SignedIn),
    (proto::SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC.procedure,  Access::SignedIn),
    (proto::SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC.procedure,     Access::Anyone),
    (proto::SIGN_IN_SERVICE_ACCEPT_INVITE_SPEC.procedure,         Access::Anyone),
    (proto::LIBRARY_SERVICE_CREATE_LIBRARY_SPEC.procedure,        MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_LIST_LIBRARIES_SPEC.procedure,        Access::SignedIn),
    (proto::LIBRARY_SERVICE_GET_LIBRARY_SPEC.procedure,           MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_UPDATE_LIBRARY_SPEC.procedure,        MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_DELETE_LIBRARY_SPEC.procedure,        MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_SCAN_LIBRARY_SPEC.procedure,          MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_PREVIEW_DELETE_MEDIA_SPEC.procedure,  DELETE_MEDIA),
    (proto::LIBRARY_SERVICE_DELETE_MEDIA_SPEC.procedure,          DELETE_MEDIA),
    (proto::LIBRARY_SERVICE_LIST_TITLE_SOURCES_SPEC.procedure,    MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_LIST_ARTWORK_SPEC.procedure,         MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_SEARCH_TITLES_SPEC.procedure,           MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_SET_MATCH_SPEC.procedure,               MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_CLEAR_MATCH_SPEC.procedure,             MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_LIST_UNMATCHED_ITEMS_SPEC.procedure,    MANAGE_LIBRARIES),
    (proto::BROWSE_SERVICE_LIST_MOVIES_SPEC.procedure,            Access::SignedIn),
    (proto::BROWSE_SERVICE_GET_MOVIE_SPEC.procedure,              Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_SHOWS_SPEC.procedure,             Access::SignedIn),
    (proto::BROWSE_SERVICE_GET_SHOW_SPEC.procedure,               Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_GENRES_SPEC.procedure,            Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_EPISODES_SPEC.procedure,          Access::SignedIn),
    (proto::BROWSE_SERVICE_GET_NEXT_EPISODE_SPEC.procedure,       Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_CONTINUE_WATCHING_SPEC.procedure, Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_FEATURED_SPEC.procedure,          Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_HOME_ROWS_SPEC.procedure,         Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_HOME_ROW_TITLES_SPEC.procedure,   Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_UNMATCHED_SPEC.procedure,         Access::SignedIn),
    (proto::BROWSE_SERVICE_GET_UNMATCHED_VERSION_SPEC.procedure,  Access::SignedIn),
    (proto::SEARCH_SERVICE_SEARCH_SPEC.procedure,                 Access::SignedIn),
    (proto::WATCH_STATE_SERVICE_REPORT_PROGRESS_SPEC.procedure,   Access::SignedIn),
    (proto::WATCH_STATE_SERVICE_SET_PLAYED_SPEC.procedure,        Access::SignedIn),
    (proto::WATCH_STATE_SERVICE_SET_HIDDEN_FROM_CONTINUE_WATCHING_SPEC.procedure, Access::SignedIn),
    (proto::WATCHLIST_SERVICE_ADD_TO_WATCHLIST_SPEC.procedure,    Access::SignedIn),
    (proto::WATCHLIST_SERVICE_REMOVE_FROM_WATCHLIST_SPEC.procedure, Access::SignedIn),
    (proto::PLAYBACK_SERVICE_PLAN_PLAYBACK_SPEC.procedure,        Access::SignedIn),
    (proto::PLAYBACK_SERVICE_STOP_PLAYBACK_SPEC.procedure,        Access::SignedIn),
    (proto::PLAYBACK_SERVICE_REPORT_TRACK_PICK_SPEC.procedure,    Access::SignedIn),
    (proto::PLAYBACK_SERVICE_GET_TRACK_PREFERENCES_SPEC.procedure, Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SET_SUBTITLE_PREFERENCE_SPEC.procedure, Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SET_AUDIO_PREFERENCE_SPEC.procedure, Access::SignedIn),
    (proto::PLAYBACK_SERVICE_GET_AUTOPLAY_SPEC.procedure,         Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SET_AUTOPLAY_SPEC.procedure,         Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SEARCH_SUBTITLES_SPEC.procedure,     DOWNLOAD_SUBTITLES),
    (proto::PLAYBACK_SERVICE_DOWNLOAD_SUBTITLE_SPEC.procedure,    DOWNLOAD_SUBTITLES),
    (proto::PLAYBACK_SERVICE_GET_SUBTITLE_APPEARANCE_SPEC.procedure, Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SET_SUBTITLE_SIZE_SPEC.procedure,    Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SET_SUBTITLE_BACKGROUND_SPEC.procedure, Access::SignedIn),
    (proto::PLAYBACK_SERVICE_SET_SUBTITLE_OFFSET_SPEC.procedure,  Access::SignedIn),
    (proto::EVENT_SERVICE_SUBSCRIBE_SPEC.procedure,               Access::SignedIn),
    (proto::USER_SERVICE_CREATE_USER_SPEC.procedure,              MANAGE_USERS),
    (proto::USER_SERVICE_LIST_USERS_SPEC.procedure,               MANAGE_USERS),
    (proto::USER_SERVICE_SET_PERMISSIONS_SPEC.procedure,          MANAGE_USERS),
    (proto::USER_SERVICE_DELETE_USER_SPEC.procedure,              MANAGE_USERS),
    (proto::USER_SERVICE_LIST_DEVICES_SPEC.procedure,             Access::SignedIn),
    (proto::USER_SERVICE_REVOKE_DEVICE_SPEC.procedure,            Access::SignedIn),
    (proto::USER_SERVICE_CHANGE_PASSWORD_SPEC.procedure,          Access::SignedIn),
    (proto::USER_SERVICE_RESET_PASSWORD_SPEC.procedure,           MANAGE_USERS),
    (proto::USER_SERVICE_GET_DEFAULT_PERMISSIONS_SPEC.procedure,  MANAGE_USERS),
    (proto::USER_SERVICE_SET_DEFAULT_PERMISSIONS_SPEC.procedure,  MANAGE_USERS),
    (proto::USER_SERVICE_CREATE_INVITE_SPEC.procedure,            MANAGE_USERS),
    (proto::USER_SERVICE_LIST_INVITES_SPEC.procedure,             MANAGE_USERS),
    (proto::USER_SERVICE_REVOKE_INVITE_SPEC.procedure,            MANAGE_USERS),
    (proto::USER_SERVICE_LIST_PLUGIN_CONNECTIONS_SPEC.procedure,  Access::SignedIn),
    (proto::USER_SERVICE_START_PLUGIN_CONNECTION_SPEC.procedure,  Access::SignedIn),
    (proto::USER_SERVICE_DELETE_PLUGIN_CONNECTION_SPEC.procedure, Access::SignedIn),
    (proto::USER_SERVICE_LIST_PERSONAL_SETTINGS_SPEC.procedure,   Access::SignedIn),
    (proto::USER_SERVICE_UPDATE_PERSONAL_SETTINGS_SPEC.procedure, Access::SignedIn),
    (proto::USER_SERVICE_DELETE_PERSONAL_SETTINGS_SPEC.procedure, Access::SignedIn),
    (proto::USER_SERVICE_LIST_NOTIFICATION_PREFERENCES_SPEC.procedure, Access::SignedIn),
    (proto::USER_SERVICE_SET_NOTIFICATION_PREFERENCE_SPEC.procedure, Access::SignedIn),
    (proto::USER_SERVICE_GET_HOME_LAYOUT_SPEC.procedure,          Access::SignedIn),
    (proto::USER_SERVICE_SET_HOME_LAYOUT_SPEC.procedure,          Access::SignedIn),
    (proto::USER_SERVICE_REORDER_HOME_ROWS_SPEC.procedure,        Access::SignedIn),
    (proto::USER_SERVICE_RESET_HOME_LAYOUT_SPEC.procedure,        Access::SignedIn),
    (proto::REQUEST_SERVICE_CREATE_REQUEST_SPEC.procedure,        REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_CREATE_SEASON_REQUESTS_SPEC.procedure, REQUEST_TV),
    (proto::REQUEST_SERVICE_LIST_REQUESTS_SPEC.procedure,         Access::SignedIn),
    (proto::REQUEST_SERVICE_GET_REQUEST_SPEC.procedure,           Access::SignedIn),
    (proto::REQUEST_SERVICE_COUNT_PENDING_REQUESTS_SPEC.procedure, MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC.procedure,         REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LIST_MOVIE_QUALITIES_SPEC.procedure,  REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LIST_DEFAULT_QUALITIES_SPEC.procedure, Access::SignedIn),
    (proto::REQUEST_SERVICE_SET_DEFAULT_QUALITY_SPEC.procedure,   Access::SignedIn),
    (proto::REQUEST_SERVICE_LIST_SHOW_QUALITIES_SPEC.procedure,   REQUEST_TV),
    (proto::REQUEST_SERVICE_LOOK_UP_SHOW_SPEC.procedure,          REQUEST_TV),
    (proto::REQUEST_SERVICE_APPROVE_REQUEST_SPEC.procedure,       MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_DECLINE_REQUEST_SPEC.procedure,       MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_CANCEL_REQUEST_SPEC.procedure,        MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_RETRY_REQUEST_SPEC.procedure,         MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_WITHDRAW_REQUEST_SPEC.procedure,      Access::SignedIn),
    (proto::REQUEST_SERVICE_CREATE_FOLLOWED_SHOW_SPEC.procedure,  REQUEST_TV),
    (proto::REQUEST_SERVICE_APPROVE_FOLLOWED_SHOW_SPEC.procedure, MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_DECLINE_FOLLOWED_SHOW_SPEC.procedure, MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_CANCEL_FOLLOWED_SHOW_SPEC.procedure,  MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_WITHDRAW_FOLLOWED_SHOW_SPEC.procedure, Access::SignedIn),
    (proto::REQUEST_SERVICE_LIST_OFFERED_QUALITIES_SPEC.procedure, MANAGE_REQUESTS),
    (proto::PLUGIN_SERVICE_LIST_PLUGINS_SPEC.procedure,           MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_GET_PLUGIN_SPEC.procedure,             MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_UPDATE_PLUGIN_SETTINGS_SPEC.procedure, MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_ENABLE_PLUGIN_SPEC.procedure,          MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_DISABLE_PLUGIN_SPEC.procedure,         MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_TEST_PLUGIN_SETTINGS_SPEC.procedure,   MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_LIST_QUALITIES_SPEC.procedure,         MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_REPLACE_QUALITIES_SPEC.procedure,      MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_LIST_PLUGIN_LOG_LINES_SPEC.procedure,  ADMIN),
];

/// Who may request paths under each prefix, which aren't RPCs: fetching media, which the media
/// routes share, uploading or removing a Plugin's, a title's or a Library's Custom Image, and
/// posting the web app's Diagnostics. The first prefix a path starts with decides.
const PREFIXES: [(&str, Access); 4] = [
    ("/media/", Access::SignedIn),
    ("/diagnostics/", Access::SignedIn),
    ("/custom-images/plugin/", MANAGE_SETTINGS),
    ("/custom-images/", MANAGE_LIBRARIES),
];

impl Access {
    /// Who may request `path`: an RPC's `/<service>/<method>` as [`ACCESS`] lists it, or a path
    /// under one of [`PREFIXES`]. `None` if nobody may.
    fn of(path: &str) -> Option<Self> {
        let prefixed = PREFIXES.iter().find(|(prefix, _)| path.starts_with(prefix));
        if let Some(&(_, access)) = prefixed {
            return Some(access);
        }
        ACCESS
            .iter()
            .find(|(procedure, _)| *procedure == path)
            .map(|&(_, access)| access)
    }

    /// Why `caller`, or nobody, may not make a call that needs this access, if they may not.
    fn check(self, caller: Option<&Caller>) -> Result<(), ConnectError> {
        match (self, caller) {
            (Self::Anyone, _) | (Self::SignedIn, Some(_)) => Ok(()),
            (_, None) => Err(sign_in_required()),
            (Self::Needs(needed), Some(caller)) if caller.user.permissions.allow(needed) => Ok(()),
            (Self::Needs(needed), Some(_)) => Err(permission_required(needed)),
        }
    }
}

/// The refusal of a call that needs the Permission `needed`, which the caller lacks.
pub(super) fn permission_required(needed: Permission) -> ConnectError {
    let name = needed.proto_name();
    refuse(
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
        format!("this needs {name}"),
    )
    .param("permission", &name)
    .into()
}

/// The RPCs that change who can sign in as whom or delete media, by path, which need Recent
/// Authentication on top of their [`Access`] (ADR 0014, and see AGENTS.md). A list of its own, not
/// an `Access` variant: `SetPermissions` needs a Permission as well, `ChangePassword` only a
/// session.
const NEEDS_RECENT_AUTHENTICATION: [&str; 7] = [
    proto::USER_SERVICE_CHANGE_PASSWORD_SPEC.procedure,
    proto::USER_SERVICE_RESET_PASSWORD_SPEC.procedure,
    proto::USER_SERVICE_SET_PERMISSIONS_SPEC.procedure,
    // It can put ADMIN in what `CreateUser`, which needs none, gives out.
    proto::USER_SERVICE_SET_DEFAULT_PERMISSIONS_SPEC.procedure,
    proto::USER_SERVICE_DELETE_USER_SPEC.procedure,
    proto::SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC.procedure,
    // Deleting can't be undone, so someone at an unlocked Device mustn't do it unasked (ADR 0038).
    proto::LIBRARY_SERVICE_DELETE_MEDIA_SPEC.procedure,
];

/// Why `caller` may not request `path` yet, if it needs Recent Authentication their Device lacks.
fn check_recent_authentication(path: &str, caller: Option<&Caller>) -> Result<(), ConnectError> {
    let lacking = caller.is_some_and(|caller| !caller.device.authenticated_recently(now()));
    if lacking && NEEDS_RECENT_AUTHENTICATION.contains(&path) {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
            "sign in again to do this",
        )
        .into());
    }
    Ok(())
}

/// Refuses `request`, before any handler sees it, unless the caller
/// [`identify_caller`](crate::users::identify_caller) found may.
pub(super) async fn require_access(request: Request, next: Next) -> axum::response::Response {
    let caller = request.extensions().get::<Caller>();
    let path = request.uri().path();
    let allowed = Access::of(path)
        .ok_or_else(|| unlisted(path))
        .and_then(|access| access.check(caller))
        .and_then(|()| check_recent_authentication(path, caller));
    match allowed {
        Ok(()) => next.run(request).await,
        Err(refused) => {
            diagnostics::rpc_failed(request.extensions(), &refused);
            refused
                .into_http_response(request.headers())
                .into_response()
        }
    }
}

/// The refusal of a path [`ACCESS`] doesn't list, session or not. An RPC that exists but isn't
/// listed is the Server's bug, not the client's, so this is the router's own 404 for a path it
/// doesn't serve, without a Refusal.
fn unlisted(path: &str) -> ConnectError {
    // connectrpc's router words its 404 this way, naming the path without its leading slash.
    let path = path.strip_prefix('/').unwrap_or(path);
    ConnectError::unimplemented(format!("method not found: {path}"))
        .with_http_status(StatusCode::NOT_FOUND)
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
