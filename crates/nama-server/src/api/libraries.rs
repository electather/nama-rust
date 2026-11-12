//! The Server's Libraries: `nama.v1.LibraryService`.

use std::path::Path;

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, CreateLibraryRequest, CreateLibraryResponse, DeleteLibraryRequest,
    DeleteLibraryResponse, DeleteMediaRequest, DeleteMediaResponse, EpisodeTakenAlong,
    FileNotDeleted, GetLibraryRequest, GetLibraryResponse, LibraryKind, LibraryService,
    ListLibrariesRequest, ListLibrariesResponse, MediaFile, MediaToDelete, Permission,
    PreviewDeleteMediaRequest, PreviewDeleteMediaResponse, RefusalReason, Request, RequestToCancel,
    ScanLibraryRequest, ScanLibraryResponse, UpdateLibraryRequest, UpdateLibraryResponse,
    media_to_delete, request::MovieOrSeason,
};

use crate::{
    browse::{self, Browse, Target, ToDelete},
    contents::Episode,
    libraries::{self, FolderProblem, Kind, Libraries, Library},
    scanner::{self, Deleted, Scanner},
};

use super::{caller, database_failed, proto_number, refuse, required_id, unknown_enum_value};

/// `nama.v1.LibraryService`: managing the Server's Libraries. Needs `PERMISSION_MANAGE_LIBRARIES`,
/// which [`require_access`](super::require_access) enforces, except listing them, which any
/// signed-in User may do to browse, and deleting media, which needs `PERMISSION_DELETE_MEDIA`.
pub(super) struct LibraryServiceHandler {
    /// Only reads (see AGENTS.md, "One path for Library changes").
    pub(super) libraries: Libraries,
    pub(super) scanner: Scanner,
    /// Finds the Versions a delete names.
    pub(super) browse: Browse,
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
        let library = self
            .scanner
            .create_library(request.name, kind, request.folders)
            .await?;
        Response::ok(CreateLibraryResponse {
            library: library_to_proto(library).into(),
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
        Response::ok(ListLibrariesResponse {
            libraries: libraries
                .into_iter()
                .map(|library| Library {
                    folders: if sees_folders {
                        library.folders
                    } else {
                        Vec::new()
                    },
                    ..library
                })
                .map(library_to_proto)
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
        Response::ok(GetLibraryResponse {
            library: library_to_proto(library).into(),
            ..Default::default()
        })
    }

    async fn update_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdateLibraryRequest>,
    ) -> ServiceResult<UpdateLibraryResponse> {
        let request = request.to_owned_message();
        let library = self
            .scanner
            .update_library(request.id, request.name, request.folders)
            .await?;
        Response::ok(UpdateLibraryResponse {
            library: library_to_proto(library).into(),
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
        self.scanner.start(request.id).await?;
        Response::ok(ScanLibraryResponse::default())
    }

    async fn preview_delete_media(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, PreviewDeleteMediaRequest>,
    ) -> ServiceResult<PreviewDeleteMediaResponse> {
        let target = to_delete_from_proto(request.to_owned_message().target.into_option())?;
        let refused = target_refused(&target);
        let mut doomed = self.browse.doomed(target).await.map_err(refused)?;
        let read_only_folder = doomed.read_only_folder().map(str::to_owned);
        let taken_along = std::mem::take(&mut doomed.taken_along);
        let planned = self.scanner.preview_delete(doomed).await;
        let planned = planned.map_err(deleting_failed)?;
        Response::ok(PreviewDeleteMediaResponse {
            bytes_freed: planned.bytes_freed(),
            files: planned.files.iter().map(file_to_proto).collect(),
            episodes_taken_along: taken_along.into_iter().map(taken_along_to_proto).collect(),
            playing: u32::try_from(planned.playing).unwrap_or(u32::MAX),
            read_only_folder,
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
        let target = to_delete_from_proto(request.to_owned_message().target.into_option())?;
        let refused = target_refused(&target);
        let doomed = self.browse.doomed(target).await.map_err(refused)?;
        let deleted = self.scanner.delete_media(doomed, by).await;
        Response::ok(deleted_to_proto(deleted.map_err(deleting_failed)?))
    }
}

fn to_delete_from_proto(target: Option<MediaToDelete>) -> Result<ToDelete, ConnectError> {
    use media_to_delete::Target as T;
    let title = |library, title| Ok(ToDelete::Title { library, title });
    match target.and_then(|target| target.target) {
        Some(T::Movie(movie)) => {
            let id = movie.movie_id.into_option();
            let id = required_id(id, "target.movie.movie_id", "Movie")?;
            title(movie.library_id, Target::Movie(id))
        }
        Some(T::VersionId(version)) => Ok(ToDelete::Version(version)),
        Some(T::Show(show)) => {
            let id = required_id(show.show_id.into_option(), "target.show.show_id", "Show")?;
            title(show.library_id, Target::Show(id))
        }
        Some(T::Season(season)) => {
            let show = season.show_id.into_option();
            let show = required_id(show, "target.season.show_id", "Show")?;
            let number = i64::from(season.season_number);
            title(season.library_id, Target::Season { show, number })
        }
        Some(T::Episode(episode)) => {
            let id = episode.episode_id.into_option();
            let id = required_id(id, "target.episode.episode_id", "Episode")?;
            match episode.version_id {
                Some(version) => Ok(ToDelete::EpisodeVersion {
                    library: episode.library_id,
                    episode: id,
                    version,
                }),
                None => title(episode.library_id, Target::Episode(id)),
            }
        }
        None => Err(refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "say what to delete",
        )
        .field("target")
        .into()),
    }
}

/// What makes the Connect error for a delete's target that Browse didn't find, naming the
/// request's fields for the Library and the Version.
fn target_refused(target: &ToDelete) -> impl FnOnce(browse::Error) -> ConnectError + use<> {
    let (library, version) = match target {
        ToDelete::Title { title, .. } => {
            let library = match title {
                Target::Movie(_) => "target.movie.library_id",
                Target::Show(_) => "target.show.library_id",
                Target::Season { .. } => "target.season.library_id",
                Target::Episode(_) => "target.episode.library_id",
            };
            (library, "target.version_id")
        }
        ToDelete::Version(_) => ("target.movie.library_id", "target.version_id"),
        ToDelete::EpisodeVersion { .. } => {
            ("target.episode.library_id", "target.episode.version_id")
        }
    };
    move |err| super::browse::refused(err, library, version)
}

fn taken_along_to_proto(episode: Episode) -> EpisodeTakenAlong {
    EpisodeTakenAlong {
        season_number: proto_number(episode.season),
        episode_number: proto_number(episode.number),
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

fn file_to_proto(file: &scanner::deleting::File) -> MediaFile {
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

/// The Connect error for a delete or its preview that was refused, or that failed on the
/// Server's side, which is logged, not sent.
fn deleting_failed(err: scanner::Error) -> ConnectError {
    let message = err.to_string();
    if let scanner::Error::ReadOnly(folder) = err {
        return refuse(RefusalReason::REFUSAL_REASON_FOLDER_READ_ONLY, message)
            .param("folder", &folder)
            .into();
    }
    tracing::error!("deleting media failed: {message}");
    ConnectError::internal("deleting failed on the Server")
}

fn kind_from_proto(kind: EnumValue<LibraryKind>) -> Result<Kind, ConnectError> {
    match kind {
        EnumValue::Known(LibraryKind::LIBRARY_KIND_MOVIES) => Ok(Kind::Movies),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_SHOWS) => Ok(Kind::Shows),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("kind", "a Library's kind must be movies or shows"),
        ),
    }
}

fn library_to_proto(library: Library) -> proto::Library {
    proto::Library {
        id: library.id,
        name: library.name,
        kind: match library.kind {
            Kind::Movies => LibraryKind::LIBRARY_KIND_MOVIES,
            Kind::Shows => LibraryKind::LIBRARY_KIND_SHOWS,
        }
        .into(),
        folders: library.folders,
        ..Default::default()
    }
}

/// The Connect error a client gets for a Library request that failed. Database failures are
/// logged, not sent: they say nothing the client can act on.
impl From<libraries::Error> for ConnectError {
    fn from(err: libraries::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            libraries::Error::NameBlank => {
                refuse(R::REFUSAL_REASON_FIELD_REQUIRED, message).field("name")
            }
            libraries::Error::FoldersEmpty => {
                refuse(R::REFUSAL_REASON_FIELD_REQUIRED, message).field("folders")
            }
            libraries::Error::Folder {
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
            libraries::Error::NotFound(id) => {
                refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message).param("libraryId", &id)
            }
            libraries::Error::FolderTaken { folder, library } => {
                refuse(R::REFUSAL_REASON_FOLDER_TAKEN, message)
                    .field("folders")
                    .param("folder", &folder)
                    .param("library", &library)
            }
            libraries::Error::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
