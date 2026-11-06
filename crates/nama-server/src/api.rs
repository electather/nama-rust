//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{future, sync::Arc};

use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_proto::nama::v1::{
    self as proto, CreateLibraryRequest, CreateLibraryResponse, DeleteLibraryRequest,
    DeleteLibraryResponse, GetLibraryRequest, GetLibraryResponse, GetServerInfoRequest,
    GetServerInfoResponse, LibraryKind, LibraryService, ListLibrariesRequest,
    ListLibrariesResponse, ScanLibraryRequest, ScanLibraryResponse, ServerService,
    UpdateLibraryRequest, UpdateLibraryResponse,
};

use crate::{
    config::Config,
    db::Database,
    libraries::{self, Kind, Libraries, Library},
    scanner::Scanner,
};

/// Routes every RPC the Server answers.
pub(crate) fn router(config: &Config, db: Arc<Database>, scanner: Scanner) -> axum::Router {
    connectrpc::Router::new()
        .add_service(Arc::new(ServerServiceHandler {
            name: config.name.clone(),
        }))
        .add_service(Arc::new(LibraryServiceHandler {
            libraries: Libraries::new(db),
            scanner,
        }))
        .into_axum_router()
}

/// `nama.v1.ServerService`: facts about this Server that any client can read.
struct ServerServiceHandler {
    name: String,
}

impl ServerService for ServerServiceHandler {
    fn get_server_info<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetServerInfoRequest>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<GetServerInfoResponse> + Send + use<'a>>> + Send
    {
        future::ready(Response::ok(GetServerInfoResponse {
            name: self.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Default::default()
        }))
    }
}

/// `nama.v1.LibraryService`: managing the Server's Libraries.
//
// TODO(M2): managing Libraries is for admins only. There are no Users yet, so every caller is
// let through; once there are, check here that the caller's Role is Admin.
struct LibraryServiceHandler {
    libraries: Libraries,
    scanner: Scanner,
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
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListLibrariesRequest>,
    ) -> ServiceResult<ListLibrariesResponse> {
        let libraries = self.libraries.list().await?;
        Response::ok(ListLibrariesResponse {
            libraries: libraries.into_iter().map(library_to_proto).collect(),
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
        self.libraries.delete(request.id).await?;
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

/// The kind a request asks for, which must be one the Server knows.
fn kind_from_proto(kind: EnumValue<LibraryKind>) -> Result<Kind, ConnectError> {
    match kind {
        EnumValue::Known(LibraryKind::LIBRARY_KIND_MOVIES) => Ok(Kind::Movies),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_SHOWS) => Ok(Kind::Shows),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            ConnectError::invalid_argument("a Library's kind must be movies or shows"),
        ),
    }
}

/// A Library as clients see it.
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
        match err {
            libraries::Error::Invalid(_) => Self::invalid_argument(err.to_string()),
            libraries::Error::NotFound(_) => Self::not_found(err.to_string()),
            libraries::Error::FolderTaken { .. } => Self::already_exists(err.to_string()),
            libraries::Error::Database(_) => {
                tracing::error!("a Library request failed: {err}");
                Self::internal("the Server's database failed")
            }
        }
    }
}
