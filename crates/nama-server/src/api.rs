//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::sync::Arc;

use axum::{http::header::SET_COOKIE, middleware};
use buffa::{EnumValue, MessageName};
use connectrpc::{ConnectError, ErrorCode, ErrorDetail, RequestContext, Response};
use nama_forwarded::identify_source;
use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{self as proto, Refusal, RefusalReason};

use crate::{
    backup::Backups,
    browse::{Browse, TitlePages},
    config::Config,
    connections::Connections,
    db::Database,
    diagnostics::{self, BrowserSpans, RecordFailures},
    events::EventBus,
    ffmpeg::{Abilities, Ffmpeg},
    home_rows::HomeRows,
    images::{ImageStore, PluginLogos, plugin_logos_router},
    library::{
        Artwork, Contents, CustomImages, ImageSources, Libraries, LocalImages, SimilarSources,
        TitleKind, TitleSources, artwork_router, custom_images_router, local_images_router,
    },
    playback::{
        Plans, PreviewFrames, Probing, RunningPlans, SubtitleCache, SubtitleDownloads,
        TrackPreferences, WholeFileReads, preview_frames_router, running_router,
    },
    plugin_settings::PluginSettings,
    requests::Requests,
    scanner::Scanner,
    server_settings,
    users::{
        Caller, SetupCode, SignIn, Users, cleared_session_cookie, identify_caller,
        in_session_cookie,
    },
};

mod access;
mod browse;
mod browser_spans;
mod events;
mod jobs;
mod libraries;
mod matches;
mod playback;
mod plugins;
mod requests;
mod search;
mod server;
mod server_link;
mod sign_in;
mod users;
mod watch_states;
mod watchlist;

pub(crate) use server_link::pull_watch_states_after_scans;

use access::require_access;

/// Routes every RPC the Server answers, its artwork and its Versions, each behind the layers
/// that identify the caller and refuse calls they may not make.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a separate handle the services are built from; a struct bundling them would \
              exist only for this call"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one flat list wiring each service to its handles; split, it would be read in pieces"
)]
pub(crate) fn router(
    config: &Config,
    db: Arc<Database>,
    plugin_settings: PluginSettings,
    users: Users,
    scanner: Scanner,
    custom_images: CustomImages,
    plugin_logos: PluginLogos,
    image_store: ImageStore,
    artwork: Artwork,
    probing: Probing,
    subtitles: SubtitleCache,
    reads: WholeFileReads,
    events: EventBus,
    requests: Requests,
    home_rows: HomeRows,
    settings: server_settings::Settings,
    running: RunningPlans,
    abilities: Abilities,
    preview_frames: PreviewFrames,
    setup_code: Option<SetupCode>,
    ffmpeg: Arc<Ffmpeg>,
) -> axum::Router {
    let plugins = plugin_settings.extension_points();
    let image_settings = settings.images();
    let local_images = LocalImages::new(Arc::clone(&db), image_store.clone());
    let artwork_router = artwork_router(
        artwork.clone(),
        Contents::new(Arc::clone(&db)),
        image_settings.clone(),
    );
    let media =
        crate::playback::router(running.clone(), &db, Arc::clone(&ffmpeg), subtitles, reads)
            .merge(running_router(running.clone()))
            .merge(preview_frames_router(preview_frames.clone()));
    let images = ImageSources {
        custom_images: custom_images.clone(),
        local_images: local_images.clone(),
        artwork: artwork.clone(),
        settings: image_settings.clone(),
    };
    let connections = Connections::new(Arc::clone(&db), plugins.clone());
    let similar = SimilarSources::new(plugins.clone());
    let pages = TitlePages::new(Arc::clone(&db), requests.clone(), similar);
    let server_url = settings.server_url();
    let rpcs = connectrpc::Router::new()
        .add_service(Arc::new(server::ServerServiceHandler {
            name: config.name.clone(),
            users: users.clone(),
            settings: settings.clone(),
            abilities: abilities.clone(),
            ffmpeg,
            images: image_store.clone(),
            points: plugins.clone(),
            home_rows: home_rows.clone(),
            data_dir: config.data_dir.clone(),
            backups: Backups::new(&config.data_dir, &config.file, settings.clone()),
            exports_diagnostics: config.otlp_endpoint.is_some(),
            running: running.clone(),
        }))
        .add_service(Arc::new(sign_in::SignInServiceHandler {
            sign_in: SignIn::new(users.clone(), setup_code),
            points: plugins.clone(),
            browse: Browse::new(Arc::clone(&db)),
        }))
        .add_service(Arc::new(matches::MatchServiceHandler {
            matcher: scanner.matcher().clone(),
            metadata: TitleSources::new(plugins.clone()),
            contents: Contents::new(Arc::clone(&db)),
            libraries: Libraries::new(Arc::clone(&db)),
        }))
        .add_service(Arc::new(browse::BrowseServiceHandler {
            browse: Browse::new(Arc::clone(&db)),
            pages: pages.clone(),
            home_rows: home_rows.clone(),
            settings: settings.clone(),
            images: images.clone(),
            db: Arc::clone(&db),
        }))
        .add_service(Arc::new(search::SearchServiceHandler {
            browse: Browse::new(Arc::clone(&db)),
            requests: requests.clone(),
            images: images.clone(),
        }))
        .add_service(Arc::new(watch_states::WatchStateServiceHandler {
            browse: Browse::new(Arc::clone(&db)),
            running: running.clone(),
            points: plugins.clone(),
        }))
        .add_service(Arc::new(watchlist::WatchlistServiceHandler {
            db: Arc::clone(&db),
        }))
        .add_service(Arc::new(playback::PlaybackServiceHandler {
            plans: Plans::new(
                probing,
                abilities,
                running,
                settings.playback(),
                preview_frames,
                Arc::clone(&db),
            ),
            preferences: TrackPreferences::new(Arc::clone(&db)),
            downloads: SubtitleDownloads::new(&db, plugins.clone(), image_store),
            db: Arc::clone(&db),
        }))
        .add_service(Arc::new(requests::RequestServiceHandler {
            requests,
            pages,
            images: images.clone(),
        }))
        .add_service(Arc::new(jobs::JobServiceHandler {
            plugin_settings: plugin_settings.clone(),
            plugins: plugins.clone(),
            db: Arc::clone(&db),
        }))
        .add_service(Arc::new(plugins::PluginServiceHandler {
            plugin_settings: plugin_settings.clone(),
            settings: settings.clone(),
            plugin_logos: plugin_logos.clone(),
            db: Arc::clone(&db),
        }))
        .add_service(Arc::new(libraries::LibraryServiceHandler {
            libraries: Libraries::new(Arc::clone(&db)),
            scanner,
            contents: Contents::new(Arc::clone(&db)),
            images,
            title_sources: TitleSources::new(plugins.clone()),
            plugins: plugins.clone(),
            plugin_settings: plugin_settings.clone(),
        }))
        .add_service(Arc::new(events::EventServiceHandler {
            events,
            users: users.clone(),
        }))
        .add_service(Arc::new(users::UserServiceHandler {
            users: users.clone(),
            connections,
            plugin_settings,
            plugin_logos: plugin_logos.clone(),
            browse: Browse::new(Arc::clone(&db)),
            db,
            server_url,
            home_rows,
            settings,
            points: plugins,
        }))
        .into_axum_service()
        .with_interceptor(RecordFailures);
    axum::Router::new()
        .fallback_service(rpcs)
        .merge(artwork_router)
        .merge(custom_images_router(
            custom_images,
            artwork,
            image_settings.clone(),
        ))
        .merge(local_images_router(local_images, image_settings))
        .merge(plugin_logos_router(plugin_logos))
        .merge(media)
        .merge(browser_spans::router(
            config.otlp_endpoint.as_deref().map(BrowserSpans::new),
        ))
        .layer(middleware::from_fn(require_access))
        .layer(middleware::from_fn_with_state(users, identify_caller))
        .layer(middleware::from_fn_with_state(
            Arc::from(config.trusted_proxies.as_slice()),
            identify_source,
        ))
        .layer(middleware::from_fn(diagnostics::trace_call))
}

/// Who sent the request `ctx` is for, or `unauthenticated` if it carries no valid session.
fn caller(ctx: &RequestContext) -> Result<&Caller, ConnectError> {
    ctx.extensions()
        .get::<Caller>()
        .ok_or_else(sign_in_required)
}

/// The Connect error for a call that needs a signed-in Device, made without one.
fn sign_in_required() -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
        "sign in first",
    )
    .into()
}

/// The response to a request whose own Device was just signed out, carrying `body`. A cookie
/// session has its cookie cleared.
fn signed_out<M>(body: M, ctx: &RequestContext) -> Response<M> {
    let response = Response::new(body);
    if in_session_cookie(ctx.headers()) {
        response.with_header(SET_COOKIE, cleared_session_cookie(ctx.extensions()))
    } else {
        response
    }
}

fn title_kind_from_proto(kind: EnumValue<proto::TitleKind>) -> Result<TitleKind, ConnectError> {
    match kind {
        EnumValue::Known(proto::TitleKind::TITLE_KIND_MOVIE) => Ok(TitleKind::Movie),
        EnumValue::Known(proto::TitleKind::TITLE_KIND_SHOW) => Ok(TitleKind::Show),
        EnumValue::Known(proto::TitleKind::TITLE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("kind", "the kind of title must be movie or show"),
        ),
    }
}

/// The Kernel's id for `id`, which a request names in `field`: refused when its namespace isn't
/// lowercase ASCII letters and digits or its value is empty.
fn external_id_from_proto(id: proto::ExternalId, field: &str) -> Result<ExternalId, ConnectError> {
    ExternalId::new(&id.namespace, id.value).map_err(|error| {
        refuse(
            RefusalReason::REFUSAL_REASON_TITLE_ID_MALFORMED,
            format!("the title's id is malformed: {error}"),
        )
        .field(field)
        .into()
    })
}

/// A season or episode number as a proto `uint32`. A scan parses them from digits, so they are
/// never negative.
fn proto_number(n: i64) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// The id of the title of kind `what` that a request names in `field`, which it must name.
fn required_id(
    id: Option<proto::ExternalId>,
    field: &str,
    what: &str,
) -> Result<ExternalId, ConnectError> {
    // An id with neither part set is one the client left out.
    match id.filter(|id| !id.namespace.is_empty() || !id.value.is_empty()) {
        Some(id) => external_id_from_proto(id, field),
        None => Err(refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            format!("the {what}'s id is missing"),
        )
        .field(field)
        .into()),
    }
}

fn external_id_to_proto(id: &ExternalId) -> proto::ExternalId {
    proto::ExternalId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..Default::default()
    }
}

/// A refusal of a client's request on its way to becoming a [`ConnectError`]: [`refuse`] starts
/// one, [`Refused::field`] and [`Refused::param`] fill it in.
struct Refused {
    code: ErrorCode,
    refusal: Refusal,
    message: String,
}

/// Refuses a request for breaking the rule `reason` names, telling the client `message`. Every
/// error a client can cause is built here (see `api/AGENTS.md`, "Refusals are typed").
fn refuse(reason: RefusalReason, message: impl Into<String>) -> Refused {
    Refused {
        code: refusal_code(reason),
        refusal: Refusal {
            reason: reason.into(),
            ..Default::default()
        },
        message: message.into(),
    }
}

/// The Connect code `reason` always comes with: API contract, as the reasons are (ADR 0019).
#[expect(
    clippy::too_many_lines,
    reason = "one flat table of every reason's code; split, it would be read in pieces"
)]
const fn refusal_code(reason: RefusalReason) -> ErrorCode {
    use RefusalReason as R;
    match reason {
        // Never sent; a client sees it only for a reason it doesn't know.
        R::REFUSAL_REASON_UNSPECIFIED
        | R::REFUSAL_REASON_FIELD_REQUIRED
        | R::REFUSAL_REASON_UNKNOWN_ENUM_VALUE
        | R::REFUSAL_REASON_WRONG_LIBRARY_KIND
        | R::REFUSAL_REASON_DURATION_OUT_OF_RANGE
        | R::REFUSAL_REASON_UNKNOWN_TITLE
        | R::REFUSAL_REASON_NAME_LENGTH
        | R::REFUSAL_REASON_NAME_CHARACTER
        | R::REFUSAL_REASON_PASSWORD_REJECTED
        | R::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE
        | R::REFUSAL_REASON_FOLDER_NOT_FOUND
        | R::REFUSAL_REASON_FOLDER_NOT_READABLE
        | R::REFUSAL_REASON_FOLDER_NOT_WRITABLE
        | R::REFUSAL_REASON_FOLDER_NOT_UTF8
        | R::REFUSAL_REASON_FOLDER_REPEATED
        | R::REFUSAL_REASON_SETTING_WRONG_KIND
        | R::REFUSAL_REASON_SETTING_OUT_OF_RANGE
        | R::REFUSAL_REASON_NOT_A_URL
        | R::REFUSAL_REASON_QUALITY_REPEATED
        | R::REFUSAL_REASON_DEFAULT_QUALITY_REQUIRED
        | R::REFUSAL_REASON_QUALITY_NOT_OFFERED
        | R::REFUSAL_REASON_PAGE_TOKEN_INVALID
        | R::REFUSAL_REASON_SHOW_HAS_QUALITY
        | R::REFUSAL_REASON_GPU_NOT_OFFERED
        | R::REFUSAL_REASON_UNKNOWN_LANGUAGE
        | R::REFUSAL_REASON_FOREIGN_AUDIO_ONLY_NOT_FULL
        | R::REFUSAL_REASON_TITLE_ID_MALFORMED
        | R::REFUSAL_REASON_TITLE_SOURCES_INVALID
        | R::REFUSAL_REASON_RECOGNISERS_INVALID
        | R::REFUSAL_REASON_SEGMENT_SOURCES_INVALID
        | R::REFUSAL_REASON_PERMISSION_NOT_REQUESTED
        | R::REFUSAL_REASON_MESSAGE_TOO_LONG
        | R::REFUSAL_REASON_SCHEDULE_INVALID => ErrorCode::InvalidArgument,
        R::REFUSAL_REASON_SIGN_IN_REQUIRED
        | R::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD
        | R::REFUSAL_REASON_WRONG_PASSWORD
        | R::REFUSAL_REASON_SIGNED_OUT => ErrorCode::Unauthenticated,
        R::REFUSAL_REASON_PERMISSION_REQUIRED
        | R::REFUSAL_REASON_WRONG_SETUP_CODE
        | R::REFUSAL_REASON_QUALITY_NOT_ALLOWED => ErrorCode::PermissionDenied,
        R::REFUSAL_REASON_TOO_MANY_ATTEMPTS
        | R::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS
        | R::REFUSAL_REASON_SERVER_BUSY => ErrorCode::ResourceExhausted,
        R::REFUSAL_REASON_ALREADY_SET_UP
        | R::REFUSAL_REASON_SIGN_IN_CODE_ALREADY_APPROVED
        | R::REFUSAL_REASON_LAST_ADMIN
        | R::REFUSAL_REASON_OWN_PASSWORD_RESET
        | R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED
        | R::REFUSAL_REASON_ALREADY_IN_LIBRARY
        | R::REFUSAL_REASON_REQUEST_NOT_PENDING
        | R::REFUSAL_REASON_REQUEST_NOT_FAILED
        | R::REFUSAL_REASON_NOT_A_REQUESTER
        | R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_PENDING
        | R::REFUSAL_REASON_NOT_A_FOLLOWED_SHOW_REQUESTER
        | R::REFUSAL_REASON_LAST_AUTHENTICATOR
        | R::REFUSAL_REASON_SETTINGS_CHECK_FAILED
        | R::REFUSAL_REASON_CHOICE_NOT_OFFERED
        | R::REFUSAL_REASON_FULFILLER_TAKEN
        | R::REFUSAL_REASON_PLUGIN_REFUSED
        | R::REFUSAL_REASON_PLUGIN_REFUSED_TO_START
        | R::REFUSAL_REASON_FOLDER_READ_ONLY
        | R::REFUSAL_REASON_TAKES_ALONG_UNNAMED
        | R::REFUSAL_REASON_TITLE_ID_MISSING
        | R::REFUSAL_REASON_NOT_A_BLOB_STORE
        | R::REFUSAL_REASON_IMAGE_STORE_IN_USE
        | R::REFUSAL_REASON_IMAGE_STORE_SWITCHING
        | R::REFUSAL_REASON_IMAGE_STORE_FAILED_CHECK
        | R::REFUSAL_REASON_FOLDER_OVERLAPS
        | R::REFUSAL_REASON_SEGMENT_FOLDER_UNWRITABLE
        | R::REFUSAL_REASON_SERVER_URL_REQUIRED
        | R::REFUSAL_REASON_JOB_RUNNING
        | R::REFUSAL_REASON_NO_SERVER_LINKER => ErrorCode::FailedPrecondition,
        R::REFUSAL_REASON_SIGN_IN_CODE_NOT_FOUND
        | R::REFUSAL_REASON_HANDLE_NOT_FOUND
        | R::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND
        | R::REFUSAL_REASON_USER_NOT_FOUND
        | R::REFUSAL_REASON_DEVICE_NOT_FOUND
        | R::REFUSAL_REASON_LIBRARY_NOT_FOUND
        | R::REFUSAL_REASON_NOTHING_MATCHED
        | R::REFUSAL_REASON_BOX_SET_NOT_FOUND
        | R::REFUSAL_REASON_SEASON_NOT_FOUND
        | R::REFUSAL_REASON_EPISODE_NOT_FOUND
        | R::REFUSAL_REASON_VERSION_NOT_FOUND
        | R::REFUSAL_REASON_TRACK_NOT_FOUND
        | R::REFUSAL_REASON_ITEM_NOT_FOUND
        | R::REFUSAL_REASON_REQUEST_NOT_FOUND
        | R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_FOUND
        | R::REFUSAL_REASON_PLUGIN_NOT_FOUND
        | R::REFUSAL_REASON_TITLE_NOT_FOUND
        | R::REFUSAL_REASON_INVITE_NOT_FOUND
        | R::REFUSAL_REASON_JOB_NOT_FOUND => ErrorCode::NotFound,
        R::REFUSAL_REASON_USER_NAME_TAKEN
        | R::REFUSAL_REASON_LIBRARY_NAME_TAKEN
        | R::REFUSAL_REASON_FOLDER_TAKEN => ErrorCode::AlreadyExists,
        R::REFUSAL_REASON_NO_TITLE_SOURCE | R::REFUSAL_REASON_NO_SUBTITLE_SOURCE => {
            ErrorCode::Unavailable
        }
    }
}

impl Refused {
    /// Names the request field, as a path (`season.show_id`), that the refusal is about.
    fn field(mut self, field: &str) -> Self {
        field.clone_into(&mut self.refusal.field);
        self
    }

    /// Names the Plugin setting `key` that the refusal is about, as `field` and a param.
    fn setting(self, key: &str) -> Self {
        self.field(&format!("settings.{key}"))
            .param("settingKey", &key)
    }

    /// Adds `value`, which the message mentions, as the param `key`: lowerCamelCase, with the unit
    /// in the key (AIP-193).
    fn param(mut self, key: &str, value: &impl ToString) -> Self {
        self.refusal
            .params
            .insert(key.to_owned(), value.to_string());
        self
    }
}

impl From<Refused> for ConnectError {
    fn from(refused: Refused) -> Self {
        let Refused {
            code,
            refusal,
            message,
        } = refused;
        let detail = ErrorDetail {
            // A Refusal is plain strings and an enum, so it always serializes.
            debug: serde_json::to_value(&refusal).ok(),
            ..ErrorDetail::from_message(Refusal::FULL_NAME, &refusal)
        };
        Self::new(code, message).with_detail(detail)
    }
}

/// The Connect error for the enum field `field` left unset, or set to a value this Server doesn't
/// know.
fn unknown_enum_value(field: &str, message: &str) -> ConnectError {
    refuse(RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE, message)
        .field(field)
        .into()
}

/// The Connect error a client gets when the database failed: `unavailable` when every reader was
/// busy, as trying again may work, else `internal`. The failure is logged, not sent: it says
/// nothing the client can act on.
#[expect(
    clippy::needless_pass_by_value,
    reason = "by value, so callers can write `.map_err(database_failed)`"
)]
fn database_failed(err: sqlx::Error) -> ConnectError {
    tracing::error!("a request failed: {err}");
    if matches!(err, sqlx::Error::PoolTimedOut) {
        ConnectError::unavailable("the Server is busy; try again")
    } else {
        ConnectError::internal("the Server's database failed")
    }
}

#[cfg(test)]
#[path = "api/api_tests.rs"]
mod tests;
