//! The Server's Libraries: `nama.v1.LibraryService`.

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, CreateLibraryRequest, CreateLibraryResponse, DeleteLibraryRequest,
    DeleteLibraryResponse, GetLibraryRequest, GetLibraryResponse, LibraryKind, LibraryService,
    ListLibrariesRequest, ListLibrariesResponse, Permission, RefusalReason, ScanLibraryRequest,
    ScanLibraryResponse, UpdateLibraryRequest, UpdateLibraryResponse,
};

use crate::{
    db::detached,
    libraries::{self, FolderProblem, Kind, Libraries, Library},
    probing::Probing,
    scanner::Scanner,
};

use super::{caller, database_failed, refuse, unknown_enum_value};

/// `nama.v1.LibraryService`: managing the Server's Libraries. Needs `PERMISSION_MANAGE_LIBRARIES`,
/// which [`require_access`](super::require_access) enforces, except listing them, which any
/// signed-in User may do to browse.
pub(super) struct LibraryServiceHandler {
    pub(super) libraries: Libraries,
    pub(super) scanner: Scanner,
    pub(super) probing: Probing,
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
            .libraries
            .create(&request.name, kind, request.folders)
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
            .libraries
            .update(request.id, &request.name, request.folders)
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
        let (libraries, probing, id) = (self.libraries.clone(), self.probing.clone(), request.id);
        // The delete commits even if the caller stops waiting, so its cleanup must follow (ADR 0029).
        detached(async move {
            libraries.delete(id).await?;
            probing.clean_subtitles().await;
            Ok::<_, libraries::Error>(())
        })
        .await?;
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
