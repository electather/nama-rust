//! Each User's own Personal settings for a Plugin, as `UserService` lists them, and linking their
//! account at a Watch sync Plugin's tracker by device code: `StartWatchSyncLink` and
//! `PollWatchSyncLink`, with the Refusals of their errors.

use connectrpc::{ConnectError, RequestContext, Response, ServiceResult};
use nama_plugin_api::{PluginId, PointKey, watch_sync::WatchSync};
use nama_proto::nama::v1::{
    self as proto, ListPersonalSettingsResponse, PollWatchSyncLinkResponse, RefusalReason,
    StartWatchSyncLinkResponse, WatchSyncLinkState,
};

use super::{
    database_failed,
    plugins::{list_failed, logo_url, setting_to_proto},
    refuse, url_signer,
};
use crate::{
    images::PluginLogos,
    plugin_settings::{LinkError, LinkState, Personal, PluginSettings},
};

/// `user`'s Personal settings for each enabled Plugin that declares any.
pub(super) async fn list(
    plugin_settings: &PluginSettings,
    plugin_logos: &PluginLogos,
    user: i64,
    ctx: &RequestContext,
) -> ServiceResult<ListPersonalSettingsResponse> {
    let listed = (plugin_settings.list_personal(user).await).map_err(list_failed)?;
    let mut plugins = Vec::new();
    for personal in listed {
        plugins.push(personal_to_proto(plugin_logos, personal, ctx).await);
    }
    Response::ok(ListPersonalSettingsResponse {
        plugins,
        ..Default::default()
    })
}

/// A User's Personal settings for a Plugin, with its Manifest's logo as `ctx`'s caller gets it.
pub(super) async fn personal_to_proto(
    plugin_logos: &PluginLogos,
    Personal {
        plugin,
        manifest,
        values,
    }: Personal,
    ctx: &RequestContext,
) -> proto::PersonalSettings {
    proto::PersonalSettings {
        plugin_id: plugin.id.to_string(),
        logo_url: logo_url(plugin_logos, &plugin, url_signer(ctx)).await,
        settings: (manifest.personal_settings.fields.iter())
            .map(|field| setting_to_proto(field, values.get(&field.key)))
            .collect(),
        watch_sync: (manifest.extension_points).contains(&PointKey::provide::<dyn WatchSync>()),
        name: manifest.name,
        ..Default::default()
    }
}

/// Starts linking an account at the Watch sync Plugin `plugin`'s tracker.
pub(super) async fn start_watch_sync_link(
    plugin_settings: &PluginSettings,
    plugin: &str,
) -> ServiceResult<StartWatchSyncLinkResponse> {
    let plugin = PluginId::from(plugin.to_owned());
    let started = (plugin_settings.start_watch_sync_link(&plugin).await)
        .map_err(|err| link_failed(&plugin, err))?;
    Response::ok(StartWatchSyncLinkResponse {
        user_code: started.user_code,
        verification_url: started.verification_url,
        expires_in_seconds: started.expires_in_seconds,
        interval_seconds: started.interval_seconds,
        device_code: started.device_code,
        ..Default::default()
    })
}

/// Whether `user` entered the code `device_code` came with at the Watch sync Plugin `plugin`'s
/// tracker.
pub(super) async fn poll_watch_sync_link(
    plugin_settings: &PluginSettings,
    user: i64,
    plugin: &str,
    device_code: &str,
) -> ServiceResult<PollWatchSyncLinkResponse> {
    let plugin = PluginId::from(plugin.to_owned());
    let state = (plugin_settings)
        .poll_watch_sync_link(user, &plugin, device_code.to_owned())
        .await
        .map_err(|err| link_failed(&plugin, err))?;
    let state = match state {
        LinkState::Pending => WatchSyncLinkState::WATCH_SYNC_LINK_STATE_PENDING,
        LinkState::Linked => WatchSyncLinkState::WATCH_SYNC_LINK_STATE_LINKED,
        LinkState::Expired => WatchSyncLinkState::WATCH_SYNC_LINK_STATE_EXPIRED,
    };
    Response::ok(PollWatchSyncLinkResponse {
        state: state.into(),
        ..Default::default()
    })
}

/// The error for linking an account at the Watch sync Plugin `plugin` failing with `err`.
fn link_failed(plugin: &PluginId, err: LinkError) -> ConnectError {
    match err {
        LinkError::NoWatchSync => refuse(
            RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
            format!("no enabled Plugin {plugin} is a Watch sync"),
        )
        .field("plugin_id")
        .param("pluginId", plugin)
        .into(),
        LinkError::Failed(_) => {
            tracing::warn!(%plugin, "linking an account at a Watch sync's tracker failed: {err}");
            ConnectError::unavailable("the Plugin couldn't ask its tracker")
        }
        LinkError::Database(err) => database_failed(err),
    }
}
