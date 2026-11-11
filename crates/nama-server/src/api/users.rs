//! The Server's Users, their Devices and Plugin Connections: `nama.v1.UserService`.

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    self as proto, ChangePasswordRequest, ChangePasswordResponse, CreateUserRequest,
    CreateUserResponse, DeletePluginConnectionRequest, DeletePluginConnectionResponse,
    DeleteUserRequest, DeleteUserResponse, GetDefaultPermissionsRequest,
    GetDefaultPermissionsResponse, ListDevicesRequest, ListDevicesResponse,
    ListPluginConnectionsRequest, ListPluginConnectionsResponse, ListUsersRequest,
    ListUsersResponse, Permission, PluginConnection, PluginConnectionState, RefusalReason,
    ResetPasswordRequest, ResetPasswordResponse, RevokeDeviceRequest, RevokeDeviceResponse,
    SetDefaultPermissionsRequest, SetDefaultPermissionsResponse, SetPermissionsRequest,
    SetPermissionsResponse, StartPluginConnectionRequest, StartPluginConnectionResponse,
    UserService,
};

use crate::{
    connections::{self, Connections},
    users::{self, Caller, Permissions, Users},
};

use super::{caller, database_failed, permission_required, refuse, signed_out, unknown_enum_value};

/// `nama.v1.UserService`: managing the Server's Users, their Devices and Plugin Connections. Needs
/// `PERMISSION_MANAGE_USERS`, which [`require_access`](super::require_access) enforces, but for
/// the self-service RPCs [`ACCESS`](super::ACCESS) opens to any session, which check for
/// themselves with [`devices_owner`] or act only on the caller.
pub(super) struct UserServiceHandler {
    pub(super) users: Users,
    pub(super) connections: Connections,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl UserService for UserServiceHandler {
    async fn create_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateUserRequest>,
    ) -> ServiceResult<CreateUserResponse> {
        let request = request.to_owned_message();
        let id = self
            .users
            .create(&caller(&ctx)?.user, &request.name, &request.password)
            .await?;
        Response::ok(CreateUserResponse {
            id,
            ..Default::default()
        })
    }

    async fn list_users(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListUsersRequest>,
    ) -> ServiceResult<ListUsersResponse> {
        let users = self.users.list().await.map_err(database_failed)?;
        Response::ok(ListUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn set_permissions(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetPermissionsRequest>,
    ) -> ServiceResult<SetPermissionsResponse> {
        let request = request.to_owned_message();
        let permissions = permissions_from_proto(request.permissions)?;
        self.users
            .set_permissions(&caller(&ctx)?.user, request.user_id, permissions)
            .await?;
        Response::ok(SetPermissionsResponse::default())
    }

    async fn delete_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeleteUserRequest>,
    ) -> ServiceResult<DeleteUserResponse> {
        let caller = caller(&ctx)?;
        self.users.delete(&caller.user, request.user_id).await?;
        let response = DeleteUserResponse::default();
        Ok(if request.user_id == caller.user.id {
            signed_out(response, &ctx)
        } else {
            Response::new(response)
        })
    }

    async fn list_devices(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListDevicesRequest>,
    ) -> ServiceResult<ListDevicesResponse> {
        let caller = caller(&ctx)?;
        let owner = devices_owner(caller, request.user_id)?;
        let devices = self
            .users
            .devices(owner)
            .await
            .map_err(database_failed)?
            .ok_or_else(no_such_user)?;
        let current = caller.device.id;
        Response::ok(ListDevicesResponse {
            devices: devices
                .into_iter()
                .map(|device| {
                    let is_current = device.id == current;
                    device_to_proto(device, is_current)
                })
                .collect(),
            ..Default::default()
        })
    }

    async fn revoke_device(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RevokeDeviceRequest>,
    ) -> ServiceResult<RevokeDeviceResponse> {
        let caller = caller(&ctx)?;
        let owner = devices_owner(caller, request.user_id)?;
        let revoked = self
            .users
            .revoke(owner, request.device_id)
            .await
            .map_err(database_failed)?;
        if !revoked {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_DEVICE_NOT_FOUND,
                "that User has no such Device",
            )
            .field("device_id")
            .into());
        }
        let response = RevokeDeviceResponse::default();
        Ok(if request.device_id == caller.device.id {
            signed_out(response, &ctx)
        } else {
            Response::new(response)
        })
    }

    async fn change_password(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ChangePasswordRequest>,
    ) -> ServiceResult<ChangePasswordResponse> {
        let request = request.to_owned_message();
        self.users
            .change_password(caller(&ctx)?, &request.new_password)
            .await?;
        Response::ok(ChangePasswordResponse::default())
    }

    async fn reset_password(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ResetPasswordRequest>,
    ) -> ServiceResult<ResetPasswordResponse> {
        let request = request.to_owned_message();
        let caller = caller(&ctx)?;
        // Resetting would sign out the calling Device too; `ChangePassword` keeps it.
        if request.user_id == caller.user.id {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_OWN_PASSWORD_RESET,
                "change your own password with ChangePassword",
            )
            .field("user_id")
            .into());
        }
        self.users
            .reset_password(&caller.user, request.user_id, &request.new_password)
            .await?;
        Response::ok(ResetPasswordResponse::default())
    }

    async fn get_default_permissions(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetDefaultPermissionsRequest>,
    ) -> ServiceResult<GetDefaultPermissionsResponse> {
        let permissions = self
            .users
            .default_permissions()
            .await
            .map_err(database_failed)?;
        Response::ok(GetDefaultPermissionsResponse {
            permissions: permissions_to_proto(permissions),
            ..Default::default()
        })
    }

    async fn set_default_permissions(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetDefaultPermissionsRequest>,
    ) -> ServiceResult<SetDefaultPermissionsResponse> {
        let permissions = permissions_from_proto(request.to_owned_message().permissions)?;
        self.users
            .set_default_permissions(&caller(&ctx)?.user, permissions)
            .await?;
        Response::ok(SetDefaultPermissionsResponse::default())
    }

    async fn list_plugin_connections(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListPluginConnectionsRequest>,
    ) -> ServiceResult<ListPluginConnectionsResponse> {
        let listed = self
            .connections
            .list(caller(&ctx)?.user.id)
            .await
            .map_err(database_failed)?;
        let connection = |(plugin, state): (PluginId, connections::State)| PluginConnection {
            plugin_id: plugin.to_string(),
            state: match state {
                connections::State::NotConnected => {
                    PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED
                }
                connections::State::Waiting => {
                    PluginConnectionState::PLUGIN_CONNECTION_STATE_WAITING
                }
                connections::State::Connected => {
                    PluginConnectionState::PLUGIN_CONNECTION_STATE_CONNECTED
                }
            }
            .into(),
            ..Default::default()
        };
        Response::ok(ListPluginConnectionsResponse {
            connections: listed.into_iter().map(connection).collect(),
            ..Default::default()
        })
    }

    async fn start_plugin_connection(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StartPluginConnectionRequest>,
    ) -> ServiceResult<StartPluginConnectionResponse> {
        let plugin = PluginId::from(request.plugin_id.to_owned());
        let url = match self.connections.start(caller(&ctx)?.user.id, &plugin).await {
            Ok(url) => url,
            Err(connections::StartError::NoConnector) => {
                return Err(refuse(
                    RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
                    format!("no enabled Plugin {plugin} connects Users"),
                )
                .field("plugin_id")
                .param("pluginId", &plugin)
                .into());
            }
            Err(err) => {
                tracing::error!(%plugin, "starting a Plugin Connection failed: {err}");
                return Err(ConnectError::unavailable(
                    "the Plugin couldn't make the link",
                ));
            }
        };
        Response::ok(StartPluginConnectionResponse {
            url,
            ..Default::default()
        })
    }

    async fn delete_plugin_connection(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeletePluginConnectionRequest>,
    ) -> ServiceResult<DeletePluginConnectionResponse> {
        let plugin = PluginId::from(request.plugin_id.to_owned());
        self.connections
            .delete(caller(&ctx)?.user.id, plugin)
            .await
            .map_err(database_failed)?;
        Response::ok(DeletePluginConnectionResponse::default())
    }
}

/// The User whose Devices `caller` asks about with `user_id`: their own if it's unset. Only a User
/// who manages Users may ask about another User's. Anyone else who does gets `not_found`, as for a
/// User who doesn't exist, so they can't learn which IDs do.
fn devices_owner(caller: &Caller, user_id: Option<i64>) -> Result<i64, ConnectError> {
    match user_id {
        None => Ok(caller.user.id),
        Some(id)
            if id == caller.user.id || caller.user.permissions.allow(Permission::ManageUsers) =>
        {
            Ok(id)
        }
        Some(_) => Err(no_such_user()),
    }
}

/// The Connect error for the request's `user_id` naming nobody the caller may see.
fn no_such_user() -> ConnectError {
    users::Error::NoSuchUser.into()
}

fn permissions_from_proto(
    permissions: Vec<EnumValue<Permission>>,
) -> Result<Permissions, ConnectError> {
    permissions
        .into_iter()
        .map(|permission| match permission {
            EnumValue::Known(Permission::Unspecified) | EnumValue::Unknown(_) => {
                Err(unknown_enum_value(
                    "permissions",
                    "every Permission must be one the Server knows",
                ))
            }
            EnumValue::Known(permission) => Ok(permission),
        })
        .collect()
}

fn permissions_to_proto(permissions: Permissions) -> Vec<EnumValue<Permission>> {
    permissions.iter().map(EnumValue::from).collect()
}

pub(super) fn user_to_proto(user: users::User) -> proto::User {
    proto::User {
        id: user.id,
        name: user.name,
        permissions: permissions_to_proto(user.permissions),
        created_at: user.created_at,
        ..Default::default()
    }
}

/// A Device as clients see it, where `is_current` says whether it is the one calling.
pub(super) fn device_to_proto(device: users::Device, is_current: bool) -> proto::Device {
    proto::Device {
        id: device.id,
        name: device.name,
        created_at: device.created_at,
        last_seen_at: device.last_seen_at,
        is_current,
        ..Default::default()
    }
}

/// The Connect error a client gets for a change to the Users that failed. Server-side failures
/// are logged, not sent.
impl From<users::Error> for ConnectError {
    fn from(err: users::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            users::Error::NameLength { field, .. } => {
                refuse(R::REFUSAL_REASON_NAME_LENGTH, message)
                    .field(field)
                    .param("minLength", users::NAME_LENGTH.start())
                    .param("maxLength", users::NAME_LENGTH.end())
            }
            users::Error::PasswordRejected { field, .. } => {
                refuse(R::REFUSAL_REASON_PASSWORD_REJECTED, message).field(field)
            }
            // Only creating a User or setting up the first one takes a name to check.
            users::Error::NameTaken => {
                refuse(R::REFUSAL_REASON_USER_NAME_TAKEN, message).field("name")
            }
            // Every call that names a User names them in `user_id`.
            users::Error::NoSuchUser => {
                refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message).field("user_id")
            }
            // A polled Sign-in Code names no User.
            users::Error::ApprovingUserDeleted => refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message),
            users::Error::AlreadySetUp => refuse(R::REFUSAL_REASON_ALREADY_SET_UP, message),
            users::Error::LastAdmin => refuse(R::REFUSAL_REASON_LAST_ADMIN, message),
            users::Error::AdminRequired => return permission_required(Permission::Admin),
            users::Error::WrongNameOrPassword => {
                refuse(R::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD, message)
            }
            users::Error::WrongPassword => {
                refuse(R::REFUSAL_REASON_WRONG_PASSWORD, message).field("password")
            }
            users::Error::SignedOut | users::Error::ApprovingDeviceSignedOut => {
                refuse(R::REFUSAL_REASON_SIGNED_OUT, message)
            }
            users::Error::Database(err) => return database_failed(err),
            users::Error::NoAuthenticator
            | users::Error::Authenticator(_)
            | users::Error::Random(_) => {
                tracing::error!("a change to the Users failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}
