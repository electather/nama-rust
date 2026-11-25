//! The Server's Users, their Devices, Plugin Connections, Personal settings and Home layouts:
//! `nama.v1.UserService`.

use std::sync::Arc;

use buffa::{EnumValue, Enumeration as _};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    self as proto, ChangePasswordRequest, ChangePasswordResponse, CreateInviteRequest,
    CreateInviteResponse, CreateUserFromAccountRequest, CreateUserFromAccountResponse,
    CreateUserRequest, CreateUserResponse, DeletePersonalSettingsRequest,
    DeletePersonalSettingsResponse, DeletePluginConnectionRequest, DeletePluginConnectionResponse,
    DeleteUserRequest, DeleteUserResponse, GetDefaultPermissionsRequest,
    GetDefaultPermissionsResponse, GetHomeLayoutRequest, GetHomeLayoutResponse, GetUserLinkRequest,
    GetUserLinkResponse, HomeLayoutRow, ListDevicesRequest, ListDevicesResponse,
    ListInvitesRequest, ListInvitesResponse, ListNotificationPreferencesRequest,
    ListNotificationPreferencesResponse, ListPersonalSettingsRequest, ListPersonalSettingsResponse,
    ListPluginConnectionsRequest, ListPluginConnectionsResponse, ListUsersRequest,
    ListUsersResponse, NotificationKind, NotificationPreference, Permission, PluginConnection,
    PluginConnectionState, RefusalReason, ReorderHomeRowsRequest, ReorderHomeRowsResponse,
    ResetHomeLayoutRequest, ResetHomeLayoutResponse, ResetPasswordRequest, ResetPasswordResponse,
    RevokeDeviceRequest, RevokeDeviceResponse, RevokeInviteRequest, RevokeInviteResponse,
    SetDefaultPermissionsRequest, SetDefaultPermissionsResponse, SetHomeLayoutRequest,
    SetHomeLayoutResponse, SetNotificationPreferenceRequest, SetNotificationPreferenceResponse,
    SetPermissionsRequest, SetPermissionsResponse, StartPluginConnectionRequest,
    StartPluginConnectionResponse, StartUserLinkRequest, StartUserLinkResponse, UnlinkUserRequest,
    UnlinkUserResponse, UpdatePersonalSettingsRequest, UpdatePersonalSettingsResponse, UserService,
    home_layout_row::Row as LayoutRow, home_row::KernelOrPlugin,
};

use crate::{
    browse::Browse,
    connections::{self, Connections},
    db::Database,
    home_rows::HomeRows,
    images::PluginLogos,
    link::Linking,
    playback::limit_to_proto,
    plugin_settings::{Personal, PluginSettings},
    server_settings::{ServerUrl, Settings},
    users::{
        self, AskedHomeLayout, Caller, Permissions, User, UserHomeLayout, Users,
        notification_preferences_of, set_notification_preference,
    },
};

use super::{
    access::permission_required,
    browse::home_rows_of,
    caller, calling_device, database_failed,
    playback::cap_from_proto,
    plugins::{change_refused, list_failed, logo_url, setting_to_proto, settings_from_proto},
    refuse,
    server::{home_layout_from_proto, home_row_from_proto},
    server_link, signed_out, unknown_enum_value, url_signer,
};

/// `nama.v1.UserService`: managing the Server's Users, their Devices and Plugin Connections. Needs
/// `PERMISSION_MANAGE_USERS`, which [`require_access`](super::require_access) enforces, but for
/// the self-service RPCs `access::ACCESS` opens to any session, which check for themselves with
/// [`user_asked_about`] or act only on the caller.
pub(super) struct UserServiceHandler {
    pub(super) users: Users,
    pub(super) connections: Connections,
    /// The Plugins, and each User's Personal settings for them.
    pub(super) plugin_settings: PluginSettings,
    /// The Manifests' logos, stored as their URLs are handed out.
    pub(super) plugin_logos: PluginLogos,
    /// Where the notification preferences and Home layouts are stored.
    pub(super) db: Arc<Database>,
    /// What Invites' links start with.
    pub(super) server_url: ServerUrl,
    /// Lists the Home rows a User's Home layout shows or hides.
    pub(super) home_rows: HomeRows,
    /// Holds the default Home layout, which each User's follows but for what they change.
    pub(super) settings: Settings,
    /// Asked to Link the caller to an Account, and to Unlink a deleted User.
    pub(super) linking: Linking,
    /// Lists each Library's Recently Added row for a User's Home layout.
    pub(super) browse: Browse,
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

    async fn create_user_from_account(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateUserFromAccountRequest>,
    ) -> ServiceResult<CreateUserFromAccountResponse> {
        let by = &caller(&ctx)?.user;
        let created = self
            .users
            .create_from_account(by, request.name, request.email)
            .await;
        if matches!(created, Err(users::Error::LinkingFailed)) {
            return Err(ConnectError::unavailable(
                "the Server couldn't ask the Hub, so no User was created; try again later",
            ));
        }
        Response::ok(CreateUserFromAccountResponse {
            id: created?,
            ..Default::default()
        })
    }

    async fn list_users(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListUsersRequest>,
    ) -> ServiceResult<ListUsersResponse> {
        let users = self.users.list().await.map_err(database_failed)?;
        let caps = self.users.remote_caps().await.map_err(database_failed)?;
        let cannot_sign_in = (self.users.cannot_sign_in().await).map_err(database_failed)?;
        Response::ok(ListUsersResponse {
            users: (users.into_iter())
                .map(|user| proto::User {
                    remote_playback_limit: limit_to_proto(caps.get(&user.id).copied()).into(),
                    ..user_to_proto(user)
                })
                .collect(),
            cannot_sign_in_user_ids: cannot_sign_in,
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
        let cap = cap_from_proto(request.remote_playback_limit)?;
        self.users
            .set_permissions(&caller(&ctx)?.user, request.user_id, permissions, cap)
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
        self.linking.unlink_deleted_user(request.user_id);
        self.signed_out_if_last(&ctx).await
    }

    async fn list_devices(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListDevicesRequest>,
    ) -> ServiceResult<ListDevicesResponse> {
        let caller = caller(&ctx)?;
        let owner = user_asked_about(caller, request.user_id)?;
        let devices = self
            .users
            .devices(owner)
            .await
            .map_err(database_failed)?
            .ok_or(users::Error::NoSuchUser)?;
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
        let owner = user_asked_about(caller, request.user_id)?;
        self.users
            .revoke(&caller.user, owner, request.device_id)
            .await?;
        self.signed_out_if_last(&ctx).await
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
        let (permissions, cap) = self
            .users
            .default_permissions()
            .await
            .map_err(database_failed)?;
        Response::ok(GetDefaultPermissionsResponse {
            permissions: permissions_to_proto(permissions),
            remote_playback_limit: limit_to_proto(cap).into(),
            ..Default::default()
        })
    }

    async fn set_default_permissions(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetDefaultPermissionsRequest>,
    ) -> ServiceResult<SetDefaultPermissionsResponse> {
        let request = request.to_owned_message();
        let permissions = permissions_from_proto(request.permissions)?;
        let cap = cap_from_proto(request.remote_playback_limit)?;
        self.users
            .set_default_permissions(&caller(&ctx)?.user, permissions, cap)
            .await?;
        Response::ok(SetDefaultPermissionsResponse::default())
    }

    async fn create_invite(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateInviteRequest>,
    ) -> ServiceResult<CreateInviteResponse> {
        let request = request.to_owned_message();
        let permissions = permissions_from_proto(request.permissions)?;
        // Read before the Invite is made, so one made always gets its link.
        let link_base = self.server_url.link("invite").ok_or_else(|| {
            refuse(
                RefusalReason::REFUSAL_REASON_SERVER_URL_REQUIRED,
                "set the Server URL first, so an Invite has a link",
            )
        })?;
        let created = self
            .users
            .create_invite(&caller(&ctx)?.user, &request.note, permissions)
            .await?;
        Response::ok(CreateInviteResponse {
            invite: invite_to_proto(created.invite).into(),
            url: format!("{link_base}/{}", created.token),
            ..Default::default()
        })
    }

    async fn list_invites(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListInvitesRequest>,
    ) -> ServiceResult<ListInvitesResponse> {
        let invites = self.users.invites().await.map_err(database_failed)?;
        Response::ok(ListInvitesResponse {
            invites: invites.into_iter().map(invite_to_proto).collect(),
            server_url_set: self.server_url.link("invite").is_some(),
            ..Default::default()
        })
    }

    async fn revoke_invite(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, RevokeInviteRequest>,
    ) -> ServiceResult<RevokeInviteResponse> {
        self.users.revoke_invite(request.invite_id).await?;
        Response::ok(RevokeInviteResponse::default())
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

    async fn list_personal_settings(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListPersonalSettingsRequest>,
    ) -> ServiceResult<ListPersonalSettingsResponse> {
        let user = caller(&ctx)?.user.id;
        let listed = self
            .plugin_settings
            .list_personal(user)
            .await
            .map_err(list_failed)?;
        let mut plugins = Vec::new();
        for personal in listed {
            plugins.push(self.personal_to_proto(personal, &ctx).await);
        }
        Response::ok(ListPersonalSettingsResponse {
            plugins,
            ..Default::default()
        })
    }

    async fn update_personal_settings(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, UpdatePersonalSettingsRequest>,
    ) -> ServiceResult<UpdatePersonalSettingsResponse> {
        let request = request.to_owned_message();
        let user = caller(&ctx)?.user.id;
        let id = PluginId::from(request.plugin_id);
        let sent = settings_from_proto(&request.settings);
        let saved = self
            .plugin_settings
            .save_personal(user, &id, sent)
            .await
            .map_err(change_refused)?;
        Response::ok(UpdatePersonalSettingsResponse {
            plugin: self.personal_to_proto(saved, &ctx).await.into(),
            ..Default::default()
        })
    }

    async fn delete_personal_settings(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeletePersonalSettingsRequest>,
    ) -> ServiceResult<DeletePersonalSettingsResponse> {
        let plugin = PluginId::from(request.plugin_id.to_owned());
        self.plugin_settings
            .delete_personal(caller(&ctx)?.user.id, &plugin)
            .await
            .map_err(database_failed)?;
        Response::ok(DeletePersonalSettingsResponse::default())
    }

    async fn list_notification_preferences(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListNotificationPreferencesRequest>,
    ) -> ServiceResult<ListNotificationPreferencesResponse> {
        let preferences = notification_preferences_of(&self.db, &caller(&ctx)?.user)
            .await
            .map_err(database_failed)?
            .into_iter()
            .map(|(kind, on)| NotificationPreference {
                kind: kind.into(),
                on,
                ..Default::default()
            })
            .collect();
        Response::ok(ListNotificationPreferencesResponse {
            preferences,
            ..Default::default()
        })
    }

    async fn set_notification_preference(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetNotificationPreferenceRequest>,
    ) -> ServiceResult<SetNotificationPreferenceResponse> {
        let request = request.to_owned_message();
        let kind = match request.kind {
            EnumValue::Known(NotificationKind::Unspecified) | EnumValue::Unknown(_) => {
                return Err(unknown_enum_value(
                    "kind",
                    "the kind must be one the Server knows",
                ));
            }
            EnumValue::Known(kind) => kind,
        };
        set_notification_preference(&self.db, caller(&ctx)?.user.id, kind, request.on)
            .await
            .map_err(database_failed)?;
        Response::ok(SetNotificationPreferenceResponse::default())
    }

    async fn get_home_layout(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetHomeLayoutRequest>,
    ) -> ServiceResult<GetHomeLayoutResponse> {
        Response::ok(GetHomeLayoutResponse {
            layout: self.home_layout(&caller(&ctx)?.user).await?.into(),
            ..Default::default()
        })
    }

    async fn set_home_layout(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetHomeLayoutRequest>,
    ) -> ServiceResult<SetHomeLayoutResponse> {
        let reader = &caller(&ctx)?.user;
        let layout = request.to_owned_message().layout.into_option();
        let featured_hidden = layout.as_ref().and_then(|layout| layout.featured_hidden);
        let asked = AskedHomeLayout {
            featured_shown: featured_hidden.map(|hidden| !hidden),
            rows: home_layout_from_proto(layout)?.rows,
        };
        let seen = home_rows_of(&self.home_rows, &self.browse, reader).await?;
        let seen = seen.into_iter().map(|(row, _)| row).collect();
        let default = self.settings.now().default_home_layout;
        (UserHomeLayout::save(&self.db, reader.id, asked, seen, default).await)
            .map_err(database_failed)?;
        Response::ok(SetHomeLayoutResponse {
            layout: self.home_layout(reader).await?.into(),
            ..Default::default()
        })
    }

    async fn reorder_home_rows(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ReorderHomeRowsRequest>,
    ) -> ServiceResult<ReorderHomeRowsResponse> {
        let reader = &caller(&ctx)?.user;
        let asked = (request.to_owned_message().rows.iter())
            .map(|row| home_row_from_proto(row, "rows"))
            .collect::<Result<Vec<_>, _>>()?;
        let seen = home_rows_of(&self.home_rows, &self.browse, reader).await?;
        let seen = seen.into_iter().map(|(row, _)| row).collect();
        let default = self.settings.now().default_home_layout;
        (UserHomeLayout::reorder(&self.db, reader.id, &asked, seen, default).await)
            .map_err(database_failed)?;
        Response::ok(ReorderHomeRowsResponse {
            layout: self.home_layout(reader).await?.into(),
            ..Default::default()
        })
    }

    async fn reset_home_layout(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ResetHomeLayoutRequest>,
    ) -> ServiceResult<ResetHomeLayoutResponse> {
        (UserHomeLayout::reset(&self.db, caller(&ctx)?.user.id).await).map_err(database_failed)?;
        Response::ok(ResetHomeLayoutResponse::default())
    }

    async fn get_user_link(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetUserLinkRequest>,
    ) -> ServiceResult<GetUserLinkResponse> {
        let user = &caller(&ctx)?.user;
        server_link::get_user(&self.linking, user).await
    }

    async fn start_user_link(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, StartUserLinkRequest>,
    ) -> ServiceResult<StartUserLinkResponse> {
        let user = &caller(&ctx)?.user;
        server_link::start_user(&self.linking, user).await
    }

    async fn unlink_user(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, UnlinkUserRequest>,
    ) -> ServiceResult<UnlinkUserResponse> {
        self.users.unlink(caller(&ctx)?.user.id).await?;
        Response::ok(UnlinkUserResponse::default())
    }
}

impl UserServiceHandler {
    /// The response to a call that may have signed a User out of the calling Device: a cookie
    /// session has its cookie cleared only once the Device's last User is gone (ADR 0106).
    async fn signed_out_if_last<M: Default>(&self, ctx: &RequestContext) -> ServiceResult<M> {
        let still_signed_in = self.users.device_exists(calling_device(ctx)?.id).await;
        Ok(if still_signed_in.map_err(database_failed)? {
            Response::new(M::default())
        } else {
            signed_out(M::default(), ctx)
        })
    }

    /// `reader`'s Home layout with every Home row they could see now, as the API has it.
    async fn home_layout(&self, reader: &User) -> Result<proto::HomeLayout, ConnectError> {
        let rows = home_rows_of(&self.home_rows, &self.browse, reader).await?;
        let own = UserHomeLayout::of(&self.db, reader.id).await;
        let own = own.map_err(database_failed)?;
        let default = self.settings.now().default_home_layout;
        Ok(proto::HomeLayout {
            featured_hidden: Some(!own.shows_featured(&default)),
            rows: (own.arrange(&default, rows).into_iter())
                .map(|((_, row), shown)| seen_row_to_proto(row, shown))
                .collect(),
            ..Default::default()
        })
    }

    /// A User's personal settings for a Plugin, with its Manifest's logo as `ctx`'s caller gets it.
    async fn personal_to_proto(
        &self,
        Personal {
            plugin,
            manifest,
            values,
        }: Personal,
        ctx: &RequestContext,
    ) -> proto::PersonalSettings {
        proto::PersonalSettings {
            plugin_id: plugin.id.to_string(),
            logo_url: logo_url(&self.plugin_logos, &plugin, url_signer(ctx)).await,
            settings: (manifest.personal_settings.fields.iter())
                .map(|field| setting_to_proto(field, values.get(&field.key)))
                .collect(),
            name: manifest.name,
            ..Default::default()
        }
    }
}

/// A Home row a User sees, as their Home layout has it.
fn seen_row_to_proto(row: KernelOrPlugin, shown: bool) -> HomeLayoutRow {
    let row = match row {
        KernelOrPlugin::Kernel(kernel) => LayoutRow::Kernel(kernel),
        KernelOrPlugin::Plugin(plugin) => LayoutRow::Plugin(plugin),
        KernelOrPlugin::RecentlyAdded(library) => LayoutRow::RecentlyAdded(library),
    };
    HomeLayoutRow {
        row: Some(row),
        hidden: !shown,
        ..Default::default()
    }
}

/// The User `caller` asks about with `user_id`, such as whose Devices to list: their own if it's
/// unset. Only a User who manages Users may ask about another User. Anyone else who does gets
/// `not_found`, as for a User who doesn't exist, so they can't learn which IDs do.
pub(super) fn user_asked_about(caller: &Caller, user_id: Option<i64>) -> Result<i64, ConnectError> {
    match user_id {
        None => Ok(caller.user.id),
        Some(id)
            if id == caller.user.id || caller.user.permissions.allow(Permission::ManageUsers) =>
        {
            Ok(id)
        }
        Some(_) => Err(users::Error::NoSuchUser.into()),
    }
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

pub(super) fn user_to_proto(user: User) -> proto::User {
    proto::User {
        id: user.id,
        name: user.name,
        permissions: permissions_to_proto(user.permissions),
        created_at: user.created_at,
        ..Default::default()
    }
}

fn invite_to_proto(invite: users::Invite) -> proto::Invite {
    proto::Invite {
        id: invite.id,
        note: invite.note,
        creator_name: invite.creator_name,
        permissions: permissions_to_proto(invite.permissions),
        created_at: invite.created_at,
        expires_at: invite.expires_at,
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
            users::Error::NameCharacter {
                field, character, ..
            } => refuse(R::REFUSAL_REASON_NAME_CHARACTER, message)
                .field(field)
                .param("character", &format!("U+{:04X}", u32::from(character))),
            users::Error::PasswordRejected { field, .. } => {
                refuse(R::REFUSAL_REASON_PASSWORD_REJECTED, message).field(field)
            }
            users::Error::SwitchPinInvalid => refuse(R::REFUSAL_REASON_SWITCH_PIN_INVALID, message)
                .field("switch_pin")
                .param("minLength", users::SWITCH_PIN_LENGTH.start())
                .param("maxLength", users::SWITCH_PIN_LENGTH.end()),
            // Only creating a User or setting up the first one takes a name to check.
            users::Error::NameTaken => {
                refuse(R::REFUSAL_REASON_USER_NAME_TAKEN, message).field("name")
            }
            // Every call that names a User names them in `user_id`.
            users::Error::NoSuchUser => {
                refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message).field("user_id")
            }
            users::Error::NoSuchDevice => {
                refuse(R::REFUSAL_REASON_DEVICE_NOT_FOUND, message).field("device_id")
            }
            users::Error::InviteNotFound { field } => {
                refuse(R::REFUSAL_REASON_INVITE_NOT_FOUND, message).field(field)
            }
            // Only making an Invite takes Permissions to grant.
            users::Error::NotGrantable(permission) => {
                refuse(R::REFUSAL_REASON_PERMISSION_REQUIRED, message)
                    .field("permissions")
                    .param("permission", &permission.proto_name())
            }
            // A polled Sign-in Code names no User.
            users::Error::ApprovingUserDeleted => refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message),
            users::Error::AlreadySetUp => refuse(R::REFUSAL_REASON_ALREADY_SET_UP, message),
            users::Error::LastAdmin => refuse(R::REFUSAL_REASON_LAST_ADMIN, message),
            users::Error::LastSignInMethod => {
                refuse(R::REFUSAL_REASON_LAST_SIGN_IN_METHOD, message)
            }
            users::Error::NoLinkingPlugin => return server_link::no_linking_plugin(),
            users::Error::LinkingFailed => {
                return Self::unavailable(server_link::LINKING_FAILED);
            }
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
            // Only creating a User from an Account runs in a task of its own.
            users::Error::Panicked(_) => {
                tracing::error!("{err}");
                return Self::internal("the Server failed to create the User");
            }
            users::Error::NoAuthenticator
            | users::Error::Authenticator(_)
            | users::Error::SwitchPinHashing(_)
            | users::Error::Random(_) => {
                tracing::error!("a change to the Users failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}
