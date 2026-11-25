//! Each User's own Personal settings for a Plugin, as `UserService` lists them.

use connectrpc::{RequestContext, Response, ServiceResult};
use nama_proto::nama::v1::{self as proto, ListPersonalSettingsResponse};

use super::{
    plugins::{list_failed, logo_url, setting_to_proto},
    url_signer,
};
use crate::{
    images::PluginLogos,
    plugin_settings::{Personal, PluginSettings},
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

/// A User's personal settings for a Plugin, with its Manifest's logo as `ctx`'s caller gets it.
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
        name: manifest.name,
        ..Default::default()
    }
}
