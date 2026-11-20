//! The Server's Libraries: `nama.v1.LibraryService`.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, CreateLibraryRequest, CreateLibraryResponse, DeleteLibraryRequest,
    DeleteLibraryResponse, DeleteMediaRequest, DeleteMediaResponse, EpisodeTakenAlong,
    FileNotDeleted, GetLibraryRequest, GetLibraryResponse, LibraryService, ListArtworkRequest,
    ListArtworkResponse, ListLibrariesRequest, ListLibrariesResponse, ListTitleSourcesRequest,
    ListTitleSourcesResponse, MediaFile, MediaToDelete, Permission, PreviewDeleteMediaRequest,
    PreviewDeleteMediaResponse, RefusalReason, Request, RequestToCancel, ScanLibraryRequest,
    ScanLibraryResponse, TitleSource, UpdateLibraryRequest, UpdateLibraryResponse, media_to_delete,
    request::MovieOrSeason,
};

use crate::{
    library::{
        Contents, FolderProblem, Libraries, Library, LibraryError, LibraryKind, NewestBackdrop,
        Order, Role, Slot, TitleSources, TitleTarget as Target, artwork_url,
    },
    plugin_settings::PluginSettings,
    plugins::ExtensionPoints,
    scanner::{self, Deleted, Scanner, TakenAlong, ToDelete},
};

use super::{
    ImageSources, Refused, ShownImages,
    browse::{Fields, not_found},
    caller, database_failed, external_id_from_proto, external_id_to_proto, proto_number, refuse,
    required_id, title_kind_from_proto, unknown_enum_value,
};

/// `nama.v1.LibraryService`: managing the Server's Libraries. Needs `PERMISSION_MANAGE_LIBRARIES`,
/// which [`require_access`](super::require_access) enforces, except listing them, which any
/// signed-in User may do to browse, and deleting media, which needs `PERMISSION_DELETE_MEDIA`.
pub(super) struct LibraryServiceHandler {
    /// Only reads (see AGENTS.md, "One path for Library changes").
    pub(super) libraries: Libraries,
    pub(super) scanner: Scanner,
    /// Finds each Library's newest title's Backdrop.
    pub(super) contents: Contents,
    pub(super) images: ImageSources,
    /// Lists the title sources a Library may name.
    pub(super) title_sources: TitleSources,
    /// Names the Plugins serving them.
    pub(super) plugins: ExtensionPoints,
    /// Says what each title source keeps.
    pub(super) plugin_settings: PluginSettings,
}

/// What each Library is shown by: see [`Backdrops::of`].
struct Backdrops {
    newest: HashMap<i64, NewestBackdrop>,
    images: ShownImages,
}

impl LibraryServiceHandler {
    // ponytail: reads every Library's newest Backdrop and every Custom Image, even for one
    // Library; both are one row per Library or upload. Query by Library if that ever grows.
    async fn backdrops(&self) -> Result<Backdrops, ConnectError> {
        Ok(Backdrops {
            newest: (self.contents.newest_backdrops().await).map_err(database_failed)?,
            images: self.images.shown().await?,
        })
    }
}

impl Backdrops {
    /// The URL of `library`'s Backdrop, and whether it is the Library's Custom Image: that, or
    /// else the Backdrop of its newest title with one, as the title's page shows it.
    fn of(&self, library: &Library) -> (Option<String>, bool) {
        if let Some(url) = self.images.custom.of(&Slot::library(library.id)) {
            return (Some(url), true);
        }
        let newest = self.newest.get(&library.id).map(|newest| {
            let (title, ids) = (&newest.title, &newest.ids);
            let shown = match library.kind {
                LibraryKind::Movies => self.images.movie_image(title, ids, Role::Backdrop),
                LibraryKind::Shows => self.images.show_image(title, ids, Role::Backdrop),
            };
            shown.unwrap_or_else(|| artwork_url(&newest.plugin, &newest.path))
        });
        (newest, false)
    }
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl LibraryService for LibraryServiceHandler {
    async fn create_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, CreateLibraryRequest>,
    ) -> ServiceResult<CreateLibraryResponse> {
        let request = request.to_owned_message();
        let kind = kind_from_proto(request.kind)?;
        let order = Order {
            title_sources: request.title_sources,
            recognisers: Some(request.recogniser_ids),
            segment_sources: Some(request.segment_source_ids),
        };
        let library = (self.scanner)
            .create_library(request.name, kind, request.folders, order)
            .await
            .map_err(changing_failed(["recogniser_ids", "segment_source_ids"]))?;
        let backdrops = self.backdrops().await?;
        Response::ok(CreateLibraryResponse {
            library: library_to_proto(library, &backdrops).into(),
            ..Default::default()
        })
    }

    async fn list_libraries(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListLibrariesRequest>,
    ) -> ServiceResult<ListLibrariesResponse> {
        // Disk paths are for those who manage Libraries (see AGENTS.md).
        let sees_folders = caller(&ctx)?
            .user
            .permissions
            .allow(Permission::ManageLibraries);
        let libraries = self.libraries.list().await?;
        let backdrops = self.backdrops().await?;
        Response::ok(ListLibrariesResponse {
            libraries: libraries
                .into_iter()
                .map(|library| {
                    if sees_folders {
                        library
                    } else {
                        Library {
                            folders: Vec::new(),
                            empty_folders: Vec::new(),
                            ..library
                        }
                    }
                })
                .map(|library| library_to_proto(library, &backdrops))
                .collect(),
            ..Default::default()
        })
    }

    async fn get_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetLibraryRequest>,
    ) -> ServiceResult<GetLibraryResponse> {
        let library = self.libraries.get(request.id).await?;
        let backdrops = self.backdrops().await?;
        Response::ok(GetLibraryResponse {
            library: library_to_proto(library, &backdrops).into(),
            ..Default::default()
        })
    }

    async fn update_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdateLibraryRequest>,
    ) -> ServiceResult<UpdateLibraryResponse> {
        let request = request.to_owned_message();
        let order = Order {
            title_sources: request.title_sources,
            recognisers: (request.recogniser_order.into_option()).map(|order| order.ids),
            segment_sources: (request.segment_source_order.into_option()).map(|order| order.ids),
        };
        let library = (self.scanner)
            .update_library(request.id, request.name, request.folders, order)
            .await
            .map_err(changing_failed([
                "recogniser_order",
                "segment_source_order",
            ]))?;
        let backdrops = self.backdrops().await?;
        Response::ok(UpdateLibraryResponse {
            library: library_to_proto(library, &backdrops).into(),
            ..Default::default()
        })
    }

    async fn delete_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, DeleteLibraryRequest>,
    ) -> ServiceResult<DeleteLibraryResponse> {
        self.scanner.delete_library(request.id).await?;
        Response::ok(DeleteLibraryResponse::default())
    }

    async fn scan_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ScanLibraryRequest>,
    ) -> ServiceResult<ScanLibraryResponse> {
        match request.remove_missing_in {
            Some(folder) => (self.scanner.remove_missing(request.id, folder.to_owned())).await?,
            None => self.scanner.start(request.id).await?,
        }
        Response::ok(ScanLibraryResponse::default())
    }

    async fn preview_delete_media(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, PreviewDeleteMediaRequest>,
    ) -> ServiceResult<PreviewDeleteMediaResponse> {
        let (target, fields) =
            to_delete_from_proto(request.to_owned_message().target.into_option())?;
        let planned = self.scanner.preview_delete(&target).await;
        let planned = planned.map_err(deleting_failed(fields))?;
        Response::ok(PreviewDeleteMediaResponse {
            bytes_freed: planned.bytes_freed(),
            files: planned.files.iter().map(file_to_proto).collect(),
            episodes_taken_along: (planned.taken_along.into_iter())
                .map(taken_along_to_proto)
                .collect(),
            playing: u32::try_from(planned.playing).unwrap_or(u32::MAX),
            read_only_folder: planned.read_only_folder,
            requests_cancelled: planned
                .requests
                .into_iter()
                .map(to_cancel_to_proto)
                .collect(),
            ..Default::default()
        })
    }

    async fn delete_media(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeleteMediaRequest>,
    ) -> ServiceResult<DeleteMediaResponse> {
        let by = caller(&ctx)?.user.name.clone();
        let request = request.to_owned_message();
        let (target, fields) = to_delete_from_proto(request.target.into_option())?;
        let also = (request.also.into_iter())
            .map(|id| external_id_from_proto(id, "also"))
            .collect::<Result<_, _>>()?;
        let deleted = self.scanner.delete_media(target, also, by).await;
        Response::ok(deleted_to_proto(deleted.map_err(deleting_failed(fields))?))
    }

    async fn list_artwork(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListArtworkRequest>,
    ) -> ServiceResult<ListArtworkResponse> {
        let request = request.to_owned_message();
        let kind = title_kind_from_proto(request.kind)?;
        let id = required_id(request.id.into_option(), "id", "title")?;
        let listed = self.images.artwork.listed(kind, &id, request.season).await;
        Response::ok(ListArtworkResponse {
            poster_urls: listed.posters,
            backdrop_urls: listed.backdrops,
            ..Default::default()
        })
    }

    async fn list_title_sources(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListTitleSourcesRequest>,
    ) -> ServiceResult<ListTitleSourcesResponse> {
        let running = (self.title_sources.sources().await.into_iter()).map(|source| TitleSource {
            plugin_name: self.plugins.name(&source.plugin),
            plugin_id: source.plugin.to_string(),
            searches: source.searches(),
            namespace: source.namespace,
            running: true,
            ..Default::default()
        });
        let known =
            (self.plugin_settings.title_sources_by_plugin().into_iter()).map(|(plugin, source)| {
                TitleSource {
                    plugin_name: self.plugins.name(&plugin),
                    plugin_id: plugin.to_string(),
                    searches: source.searches,
                    namespace: source.namespace,
                    running: false,
                    ..Default::default()
                }
            });
        let mut listed = HashSet::new();
        Response::ok(ListTitleSourcesResponse {
            // Running first, so a namespace a running source keeps is listed as running.
            title_sources: (running.chain(known))
                .filter(|source| listed.insert(source.namespace.clone()))
                .collect(),
            ..Default::default()
        })
    }
}

/// What a delete's `target` asks for, and the request's fields naming what it looks up.
fn to_delete_from_proto(target: Option<MediaToDelete>) -> Result<(ToDelete, Fields), ConnectError> {
    use media_to_delete::Target as T;
    let title = |library, title| ToDelete::Title { library, title };
    let Some(target) = target.and_then(|target| target.target) else {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "say what to delete",
        )
        .field("target")
        .into());
    };
    Ok(match target {
        T::Movie(movie) => {
            let fields = Fields {
                library: "target.movie.library_id",
                title: "target.movie.movie_id",
                ..Fields::NONE
            };
            let id = required_id(movie.movie_id.into_option(), fields.title, "Movie")?;
            (title(movie.library_id, Target::Movie(id)), fields)
        }
        T::VersionId(version) => {
            let fields = Fields {
                version: "target.version_id",
                ..Fields::NONE
            };
            (ToDelete::Version(version), fields)
        }
        T::Show(show) => {
            let fields = Fields {
                library: "target.show.library_id",
                title: "target.show.show_id",
                ..Fields::NONE
            };
            let id = required_id(show.show_id.into_option(), fields.title, "Show")?;
            (title(show.library_id, Target::Show(id)), fields)
        }
        T::Season(season) => {
            let fields = Fields {
                library: "target.season.library_id",
                title: "target.season.show_id",
                season: "target.season.season_number",
                ..Fields::NONE
            };
            let show = required_id(season.show_id.into_option(), fields.title, "Show")?;
            let number = i64::from(season.season_number);
            (
                title(season.library_id, Target::Season { show, number }),
                fields,
            )
        }
        T::Episode(episode) => {
            let fields = Fields {
                library: "target.episode.library_id",
                title: "target.episode.episode_id",
                ..Fields::NONE
            };
            let id = required_id(episode.episode_id.into_option(), fields.title, "Episode")?;
            match episode.version_id {
                Some(version) => {
                    let fields = Fields {
                        version: "target.episode.version_id",
                        ..fields
                    };
                    let library = episode.library_id;
                    let one_version = ToDelete::EpisodeVersion {
                        library,
                        episode: id,
                        version,
                    };
                    (one_version, fields)
                }
                None => (title(episode.library_id, Target::Episode(id)), fields),
            }
        }
    })
}

fn taken_along_to_proto(taken: TakenAlong) -> EpisodeTakenAlong {
    let episode = taken.episode;
    EpisodeTakenAlong {
        season_number: proto_number(episode.season),
        episode_number: proto_number(episode.number),
        episode_id: external_id_to_proto(&episode.id).into(),
        title: episode.title,
        ..Default::default()
    }
}

/// `request` named as its Requesters know it, without who they are: a User with Delete Media
/// needn't be one who may see them.
fn to_cancel_to_proto(request: Request) -> RequestToCancel {
    let (title, season_number) = match request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => (movie.title, None),
        Some(MovieOrSeason::Season(season)) => (season.show_title, Some(season.season_number)),
        None => (String::new(), None),
    };
    RequestToCancel {
        title,
        season_number,
        ..Default::default()
    }
}

fn deleted_to_proto(deleted: Deleted) -> DeleteMediaResponse {
    DeleteMediaResponse {
        bytes_freed: deleted.bytes_freed(),
        deleted: deleted.files.iter().map(file_to_proto).collect(),
        not_deleted: (deleted.failed.into_iter())
            .map(|(path, err)| FileNotDeleted {
                file_name: file_name(&path),
                reason: err.to_string(),
                ..Default::default()
            })
            .collect(),
        fulfillers_not_told: deleted.fulfillers_not_told,
        ..Default::default()
    }
}

fn file_to_proto(file: &scanner::DoomedFile) -> MediaFile {
    MediaFile {
        file_name: file_name(&file.path),
        size: file.size,
        ..Default::default()
    }
}

/// The name of the file at `path`, without the folders it is in, which are for those who manage
/// Libraries (see AGENTS.md).
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// What makes the Connect error for a delete or its preview that was refused, naming the field
/// in `fields` of what it didn't find, or that failed on the Server's side, which is logged, not
/// sent.
fn deleting_failed(fields: Fields) -> impl Fn(scanner::Error) -> ConnectError {
    move |err| {
        let message = err.to_string();
        match err {
            scanner::Error::NotFound(err) => not_found(fields, &err).into(),
            scanner::Error::ReadOnly(folder) => {
                refuse(RefusalReason::REFUSAL_REASON_FOLDER_READ_ONLY, message)
                    .param("folder", &folder)
                    .into()
            }
            scanner::Error::TakesAlongUnnamed(taken_along) => {
                refuse(RefusalReason::REFUSAL_REASON_TAKES_ALONG_UNNAMED, message)
                    .field("also")
                    .param("episodes", &TakenAlong::names(&taken_along))
                    .into()
            }
            scanner::Error::Library(_)
            | scanner::Error::Database(_)
            | scanner::Error::Walk(_)
            | scanner::Error::Cancelled
            | scanner::Error::Requests(_) => {
                tracing::error!("deleting media failed: {message}");
                ConnectError::internal("deleting failed on the Server")
            }
        }
    }
}

fn kind_from_proto(kind: EnumValue<proto::LibraryKind>) -> Result<LibraryKind, ConnectError> {
    match kind {
        EnumValue::Known(proto::LibraryKind::LIBRARY_KIND_MOVIES) => Ok(LibraryKind::Movies),
        EnumValue::Known(proto::LibraryKind::LIBRARY_KIND_SHOWS) => Ok(LibraryKind::Shows),
        EnumValue::Known(proto::LibraryKind::LIBRARY_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
            Err(unknown_enum_value(
                "kind",
                "a Library's kind must be movies or shows",
            ))
        }
    }
}

fn library_to_proto(library: Library, backdrops: &Backdrops) -> proto::Library {
    let (backdrop_url, custom_backdrop) = backdrops.of(&library);
    proto::Library {
        id: library.id,
        name: library.name,
        kind: match library.kind {
            LibraryKind::Movies => proto::LibraryKind::LIBRARY_KIND_MOVIES,
            LibraryKind::Shows => proto::LibraryKind::LIBRARY_KIND_SHOWS,
        }
        .into(),
        folders: library.folders,
        backdrop_thumbhash: backdrops.images.thumbhash(backdrop_url.as_deref()),
        backdrop_url,
        custom_backdrop,
        title_sources: library.title_sources,
        recogniser_ids: library.recognisers,
        segment_source_ids: library.segment_sources,
        empty_folders: library.empty_folders,
        ..Default::default()
    }
}

/// The Connect error a client gets for a Library request that failed. Database failures are
/// logged, not sent: they say nothing the client can act on.
impl From<LibraryError> for ConnectError {
    fn from(err: LibraryError) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            LibraryError::NameBlank => {
                refuse(R::REFUSAL_REASON_FIELD_REQUIRED, message).field("name")
            }
            LibraryError::FoldersEmpty => {
                refuse(R::REFUSAL_REASON_FIELD_REQUIRED, message).field("folders")
            }
            LibraryError::Folder {
                index,
                folder,
                problem,
            } => {
                let reason = match problem {
                    FolderProblem::NotAbsolute => R::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE,
                    FolderProblem::NotFound(_) => R::REFUSAL_REASON_FOLDER_NOT_FOUND,
                    FolderProblem::NotReadable(_) => R::REFUSAL_REASON_FOLDER_NOT_READABLE,
                    FolderProblem::NotUtf8(_) => R::REFUSAL_REASON_FOLDER_NOT_UTF8,
                    FolderProblem::Repeated => R::REFUSAL_REASON_FOLDER_REPEATED,
                };
                let refused = refuse(reason, message)
                    .field(&format!("folders[{index}]"))
                    .param("folder", &folder);
                if let FolderProblem::NotUtf8(resolved) = problem {
                    refused.param("resolvedPath", &resolved)
                } else {
                    refused
                }
            }
            LibraryError::NotFound(id) => {
                refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message).param("libraryId", &id)
            }
            LibraryError::NameTaken { library } => {
                refuse(R::REFUSAL_REASON_LIBRARY_NAME_TAKEN, message)
                    .field("name")
                    .param("library", &library)
            }
            LibraryError::FolderTaken { folder, library } => {
                refuse(R::REFUSAL_REASON_FOLDER_TAKEN, message)
                    .field("folders")
                    .param("folder", &folder)
                    .param("library", &library)
            }
            LibraryError::FolderOverlaps {
                folder,
                library,
                library_folder,
                ..
            } => refuse(R::REFUSAL_REASON_FOLDER_OVERLAPS, message)
                .field("folders")
                .param("folder", &folder)
                .param("library", &library)
                .param("libraryFolder", &library_folder),
            LibraryError::TitleSources { namespace } => {
                let refused =
                    refuse(R::REFUSAL_REASON_TITLE_SOURCES_INVALID, message).field("title_sources");
                match namespace {
                    Some(namespace) => refused.param("namespace", &namespace),
                    None => refused,
                }
            }
            LibraryError::TitleSourceUnknown { namespace } => {
                refuse(R::REFUSAL_REASON_TITLE_SOURCES_INVALID, message)
                    .field("title_sources")
                    .param("namespace", &namespace)
            }
            LibraryError::NoTitleSourceSearches => {
                refuse(R::REFUSAL_REASON_TITLE_SOURCES_INVALID, message).field("title_sources")
            }
            // Their fields are the request's, which `changing_failed` names.
            LibraryError::RecognisersInvalid { plugin } => recognisers_invalid(message, &plugin),
            LibraryError::SegmentSourcesInvalid { plugin } => {
                segment_sources_invalid(message, &plugin)
            }
            LibraryError::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}

/// What a Library change failing tells the client, as [`From`] does, naming `fields`, the request
/// fields its Recogniser and Segment source orders came in, for a refused one.
fn changing_failed(fields: [&'static str; 2]) -> impl Fn(LibraryError) -> ConnectError {
    move |err| {
        let [recognisers_field, segment_sources_field] = fields;
        if let LibraryError::RecognisersInvalid { plugin } = &err {
            let refused = recognisers_invalid(err.to_string(), plugin);
            return refused.field(recognisers_field).into();
        }
        if let LibraryError::SegmentSourcesInvalid { plugin } = &err {
            let refused = segment_sources_invalid(err.to_string(), plugin);
            return refused.field(segment_sources_field).into();
        }
        err.into()
    }
}

/// Refuses a Segment source order for naming `plugin`, leaving the field to the caller.
fn segment_sources_invalid(message: String, plugin: &str) -> Refused {
    refuse(
        RefusalReason::REFUSAL_REASON_SEGMENT_SOURCES_INVALID,
        message,
    )
    .param("pluginId", &plugin)
}

/// Refuses a Recogniser order for naming `plugin`, leaving the field to the caller.
fn recognisers_invalid(message: String, plugin: &str) -> Refused {
    refuse(RefusalReason::REFUSAL_REASON_RECOGNISERS_INVALID, message).param("pluginId", &plugin)
}
