//! The Server's Plugins and their settings: `nama.v1.PluginService`.

use std::collections::BTreeMap;

use buffa::{EnumValue, Enumeration};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::{
    Manifest, PluginId, PointKey,
    auth::Authenticator,
    connector::Connector,
    fulfiller::{Fulfiller, Movies, Shows},
    metadata::MetadataProvider,
    notifier::Notifier,
    points::{Events, ServerStarted},
    settings::{self, SettingField, Settings, SettingsError},
};
use nama_proto::nama::v1::{
    self as proto, DisablePluginRequest, DisablePluginResponse, EnablePluginRequest,
    EnablePluginResponse, GetPluginRequest, GetPluginResponse, ListPluginsRequest,
    ListPluginsResponse, PluginService, RefusalReason, TestPluginSettingsRequest,
    TestPluginSettingsResponse, UpdatePluginSettingsRequest, UpdatePluginSettingsResponse,
};

use crate::{
    custom_images::{CustomImages, Slot, Urls},
    plugin_logos::PluginLogos,
    plugin_settings::{ChangeError, ListError, PluginSettings, StoredPlugin},
    plugins::{CheckError, EnableError, Listed},
    qualities,
};

use super::{custom_image_urls, database_failed, refuse};

/// `nama.v1.PluginService`: the Server's Plugins and their settings. Needs
/// `PERMISSION_MANAGE_SETTINGS`, which [`require_access`](super::require_access) enforces.
pub(super) struct PluginServiceHandler {
    /// Each Plugin with what is stored for it, which every change to one goes through.
    pub(super) plugin_settings: PluginSettings,
    /// The logos admins uploaded, shown in place of the Manifests'.
    pub(super) custom_images: CustomImages,
    /// The Manifests' logos, stored as their URLs are handed out.
    pub(super) plugin_logos: PluginLogos,
}

/// A Plugin's logo as clients fetch it.
struct Logo {
    /// Empty if it has none.
    url: String,
    /// Whether it is a Custom Image.
    custom: bool,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl PluginService for PluginServiceHandler {
    async fn list_plugins(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListPluginsRequest>,
    ) -> ServiceResult<ListPluginsResponse> {
        let listed = self.plugin_settings.list().await.map_err(list_failed)?;
        let custom = custom_image_urls(&self.custom_images).await?;
        let mut summaries = Vec::with_capacity(listed.len());
        for StoredPlugin { plugin, stored } in listed {
            let logo = self.logo(&plugin, &custom).await;
            summaries.push(plugin_summary_to_proto(plugin, stored.settings, logo));
        }
        Response::ok(ListPluginsResponse {
            summaries,
            ..Default::default()
        })
    }

    async fn get_plugin(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetPluginRequest>,
    ) -> ServiceResult<GetPluginResponse> {
        // Doesn't read `plugins/` again as listing does: the details page opens from the list.
        let stored = self.known(&PluginId::from(request.id.to_owned())).await?;
        Response::ok(GetPluginResponse {
            plugin: self.plugin_with_logo(stored).await?.into(),
            ..Default::default()
        })
    }

    async fn update_plugin_settings(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdatePluginSettingsRequest>,
    ) -> ServiceResult<UpdatePluginSettingsResponse> {
        let request = request.to_owned_message();
        let id = PluginId::from(request.id);
        let stored = self
            .plugin_settings
            .update(&id, settings_from_proto(&request.settings))
            .await
            .map_err(change_refused)?;
        Response::ok(UpdatePluginSettingsResponse {
            plugin: self.plugin_with_logo(stored).await?.into(),
            ..Default::default()
        })
    }

    async fn enable_plugin(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, EnablePluginRequest>,
    ) -> ServiceResult<EnablePluginResponse> {
        let id = PluginId::from(request.id.to_owned());
        self.plugin_settings
            .enable(&id)
            .await
            .map_err(change_refused)?;
        Response::ok(EnablePluginResponse::default())
    }

    async fn test_plugin_settings(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, TestPluginSettingsRequest>,
    ) -> ServiceResult<TestPluginSettingsResponse> {
        let request = request.to_owned_message();
        let choices = self
            .plugin_settings
            .check(
                &PluginId::from(request.id),
                settings_from_proto(&request.settings),
            )
            .await
            .map_err(change_refused)?
            .into_iter()
            .map(|(key, choices)| (key, setting_choices_to_proto(choices)))
            .collect();
        Response::ok(TestPluginSettingsResponse {
            choices,
            ..Default::default()
        })
    }

    async fn disable_plugin(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, DisablePluginRequest>,
    ) -> ServiceResult<DisablePluginResponse> {
        self.plugin_settings
            .disable(&PluginId::from(request.id.to_owned()))
            .await
            .map_err(change_refused)?;
        Response::ok(DisablePluginResponse::default())
    }

    async fn list_qualities(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, proto::ListQualitiesRequest>,
    ) -> ServiceResult<proto::ListQualitiesResponse> {
        let qualities = self
            .plugin_settings
            .qualities(&PluginId::from(request.id.to_owned()))
            .await
            .map_err(change_refused)?;
        Response::ok(proto::ListQualitiesResponse {
            qualities,
            ..Default::default()
        })
    }

    async fn replace_qualities(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, proto::ReplaceQualitiesRequest>,
    ) -> ServiceResult<proto::ReplaceQualitiesResponse> {
        let request = request.to_owned_message();
        let id = PluginId::from(request.id);
        let qualities = self
            .plugin_settings
            .replace_qualities(&id, request.qualities)
            .await
            .map_err(change_refused)?
            .map_err(|err| invalid_qualities(err, &id))?;
        Response::ok(proto::ReplaceQualitiesResponse {
            qualities,
            ..Default::default()
        })
    }
}

impl PluginServiceHandler {
    /// The Plugin `id`, refused or not, with what is stored for it, or the refusal of a call
    /// naming one no Plugin has.
    async fn known(&self, id: &PluginId) -> Result<StoredPlugin, ConnectError> {
        self.plugin_settings
            .get(id)
            .await
            .map_err(database_failed)?
            .ok_or_else(|| plugin_not_found(id))
    }

    /// `stored` as `Plugin`, with its logo.
    async fn plugin_with_logo(&self, stored: StoredPlugin) -> Result<proto::Plugin, ConnectError> {
        let custom = custom_image_urls(&self.custom_images).await?;
        let logo = self.logo(&stored.plugin, &custom).await;
        Ok(plugin_to_proto(stored.plugin, stored.stored.settings, logo))
    }

    /// `plugin`'s logo: its Custom Image, among `custom`, else its Manifest's, which is stored
    /// first if it isn't. None if the store fails, which is logged: the Plugin is still listed.
    async fn logo(&self, plugin: &Listed, custom: &Urls) -> Logo {
        if let Some(url) = custom.of(&Slot::plugin(&plugin.id)) {
            return Logo { url, custom: true };
        }
        let manifest_logo = plugin
            .manifest
            .as_ref()
            .ok()
            .and_then(|it| it.logo_svg.as_deref());
        let url = match manifest_logo {
            Some(svg) => self.plugin_logos.stored_url(&plugin.id, svg).await.unwrap_or_else(|err| {
                tracing::error!(plugin = %plugin.id, "could not store the Plugin's logo: {err}");
                String::new()
            }),
            None => String::new(),
        };
        Logo { url, custom: false }
    }
}

/// The refusal of a Plugin change or settings check failing with `err`. A database failure is
/// the Server's, logged and not sent.
fn change_refused(err: ChangeError) -> ConnectError {
    match err {
        ChangeError::Unknown(id) => plugin_not_found(&id),
        ChangeError::Refused { plugin, reason } => EnableError::Refused { plugin, reason }.into(),
        ChangeError::InvalidSettings(error) => settings_refused(error),
        ChangeError::NotOffered { plugin, key, value } => {
            let message = format!("{value} isn't one of the choices for the setting {key}");
            refuse(RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED, message)
                .param("pluginId", &plugin)
                .param("settingValue", &value)
                .setting(&key)
                .into()
        }
        ChangeError::Check {
            plugin,
            error,
            setting,
        } => check_refused(&plugin, error, setting.as_deref()),
        ChangeError::Enable(err) => err.into(),
        // Also when it isn't running (see `Registry::last_provider`), so not "is the only one".
        ChangeError::LastAuthenticator(last) => refuse(
            RefusalReason::REFUSAL_REASON_LAST_AUTHENTICATOR,
            format!(
                "{} can't be disabled while no other enabled Plugin lets Users sign in",
                last.plugin
            ),
        )
        .field("id")
        .param("pluginId", &last.plugin)
        .into(),
        ChangeError::Database(err) => database_failed(err),
    }
}

/// The refusal of the Plugin `id`'s settings check failing with `err`, naming the `setting` a
/// save was refused for, if any.
fn check_refused(id: &PluginId, err: CheckError, setting: Option<&str>) -> ConnectError {
    let check_failed = |message: String| {
        let refused = refuse(RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED, message)
            .param("pluginId", id);
        match setting {
            Some(key) => refused.setting(key),
            None => refused,
        }
    };
    match err {
        CheckError::Unknown(unknown) => plugin_not_found(&unknown),
        CheckError::InvalidSettings(error) => settings_refused(error),
        CheckError::Failed(message) => check_failed(message).into(),
        CheckError::TimedOut(_, after) => check_failed(err.to_string())
            .param("timeoutSeconds", &after.as_secs())
            .into(),
    }
}

/// The refusal of a call naming the Plugin `id` in `id`, which no Plugin has.
fn plugin_not_found(id: &PluginId) -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
        format!("there is no Plugin {id}"),
    )
    .field("id")
    .param("pluginId", id)
    .into()
}

/// The Connect error a client gets for a Plugin that couldn't be enabled. A Plugin that panicked
/// is the Server's failure, logged and not sent.
impl From<EnableError> for ConnectError {
    fn from(err: EnableError) -> Self {
        match err {
            EnableError::Unknown(id) => plugin_not_found(&id),
            EnableError::Refused { plugin, reason } => refuse(
                RefusalReason::REFUSAL_REASON_PLUGIN_REFUSED,
                format!("the Server refused {plugin}: {reason}"),
            )
            .field("id")
            .param("pluginId", &plugin)
            .param("reason", &reason)
            .into(),
            // The settings are the stored ones, not the request's, but #162 asks for the refusal
            // to name one.
            EnableError::InvalidSettings { error, .. } => settings_refused(error),
            EnableError::FulfillerTaken { plugin, kind, by } => refuse(
                RefusalReason::REFUSAL_REASON_FULFILLER_TAKEN,
                format!(
                    "{plugin} can't be enabled while {by} hands {kind} Requests on: disable {by} first"
                ),
            )
            .field("id")
            .param("pluginId", &plugin)
            .param("fulfillerPluginId", &by)
            .param("titleKind", &kind)
            .into(),
            EnableError::AlreadyEnabled(_)
            | EnableError::ManifestPanicked { .. }
            | EnableError::Panicked { .. } => {
                tracing::error!("enabling a plugin failed: {err}");
                Self::internal("the Server failed")
            }
        }
    }
}

/// The refusal of settings that don't fit their Plugin's schema, naming the one that doesn't as
/// `settings.<key>`, the way `Plugin.settings` lists it.
fn settings_refused(err: SettingsError) -> ConnectError {
    let refused = match err {
        SettingsError::Missing(key) => refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            format!("the setting {key} is required"),
        )
        .setting(&key),
        SettingsError::WrongKind { key, expected } => {
            let kind = proto::SettingKind::from(expected).proto_name();
            refuse(
                RefusalReason::REFUSAL_REASON_SETTING_WRONG_KIND,
                format!("the setting {key} must be {kind}"),
            )
            .setting(&key)
            .param("settingKind", &kind)
        }
    };
    refused.into()
}

fn plugin_summary_to_proto(
    plugin: Listed,
    stored: BTreeMap<String, settings::SettingValue>,
    logo: Logo,
) -> proto::PluginSummary {
    let (manifest, refused_reason) = manifest_and_refusal(&plugin.id, plugin.manifest);
    // `EnablePlugin`'s own check, so the list offers Set up exactly where enabling would be refused
    // for a missing setting.
    let settings_missing = matches!(
        Settings::new(stored).resolve(&manifest.settings),
        Err(SettingsError::Missing(_))
    );
    proto::PluginSummary {
        settings_missing,
        extension_points: extension_points_to_proto(&manifest.extension_points),
        id: plugin.id.to_string(),
        name: manifest.name,
        description: manifest.description,
        version: manifest.version,
        kind: plugin.kind.into(),
        enabled: plugin.enabled,
        refused_reason,
        logo_url: logo.url,
        custom_logo: logo.custom,
        ..Default::default()
    }
}

fn plugin_to_proto(
    plugin: Listed,
    stored: BTreeMap<String, settings::SettingValue>,
    logo: Logo,
) -> proto::Plugin {
    // A refused Plugin has no settings schema, so it comes with none.
    let settings = plugin
        .manifest
        .iter()
        .flat_map(|manifest| &manifest.settings.fields)
        .map(|field| setting_to_proto(field, stored.get(&field.key)))
        .collect();
    proto::Plugin {
        checks_settings: plugin.checks_settings,
        settings,
        summary: plugin_summary_to_proto(plugin, stored, logo).into(),
        ..Default::default()
    }
}

/// The Plugin `id`'s Manifest as `listed`, and why the Server refused it, empty if it didn't. A
/// refused Plugin has no Manifest, so it is shown with an empty one: nothing but its id.
fn manifest_and_refusal(id: &PluginId, listed: Result<Manifest, String>) -> (Manifest, String) {
    match listed {
        Ok(manifest) => (manifest, String::new()),
        Err(reason) => (Manifest::new(id.clone()), reason),
    }
}

/// The Extension Points an admin is shown for `points`, leaving out any this API doesn't name,
/// such as a test's own.
fn extension_points_to_proto(points: &[PointKey]) -> Vec<EnumValue<proto::ExtensionPoint>> {
    use proto::ExtensionPoint as Public;
    let public = [
        (
            PointKey::provide::<dyn Fulfiller<Movies>>(),
            Public::EXTENSION_POINT_MOVIE_FULFILLER,
        ),
        (
            PointKey::provide::<dyn Fulfiller<Shows>>(),
            Public::EXTENSION_POINT_SHOW_FULFILLER,
        ),
        (
            PointKey::provide::<dyn Notifier>(),
            Public::EXTENSION_POINT_NOTIFIER,
        ),
        (
            PointKey::provide::<dyn Connector>(),
            Public::EXTENSION_POINT_CONNECTOR,
        ),
        (
            PointKey::provide::<dyn Authenticator>(),
            Public::EXTENSION_POINT_AUTHENTICATOR,
        ),
        (
            PointKey::provide::<dyn MetadataProvider>(),
            Public::EXTENSION_POINT_METADATA_PROVIDER,
        ),
        (
            PointKey::observe::<Events>(),
            Public::EXTENSION_POINT_EVENTS_OBSERVER,
        ),
        (
            PointKey::observe::<ServerStarted>(),
            Public::EXTENSION_POINT_SERVER_STARTED_OBSERVER,
        ),
    ];
    points
        .iter()
        .filter_map(|point| public.iter().find(|(key, _)| key == point))
        .map(|&(_, public)| public.into())
        .collect()
}

/// A setting with its `stored` value, as a client may see it: a secret's value, stored or
/// default, never leaves the Server (see AGENTS.md, "Secrets stay out of storage and logs").
fn setting_to_proto(
    field: &SettingField,
    stored: Option<&settings::SettingValue>,
) -> proto::Setting {
    let secret = field.kind == settings::SettingKind::Secret;
    let shown = |value: Option<&settings::SettingValue>| {
        value
            .filter(|_| !secret)
            .map(proto::SettingValue::from)
            .into()
    };
    proto::Setting {
        key: field.key.clone(),
        label: field.label.clone(),
        description: field.description.clone(),
        kind: proto::SettingKind::from(field.kind).into(),
        required: field.required,
        default_value: shown(field.default.as_ref()),
        value: shown(stored),
        secret_set: secret && stored.is_some(),
        ..Default::default()
    }
}

/// The settings a request gives, by key, leaving out those [`setting_value_from_proto`] does.
fn settings_from_proto<'a>(
    sent: impl IntoIterator<Item = (&'a String, &'a proto::SettingValue)>,
) -> BTreeMap<String, settings::SettingValue> {
    sent.into_iter()
        .filter_map(|(key, value)| Some((key.clone(), setting_value_from_proto(value)?)))
        .collect()
}

fn setting_choices_to_proto(choices: Vec<settings::Choice>) -> proto::SettingChoices {
    proto::SettingChoices {
        choices: choices
            .into_iter()
            .map(|choice| proto::Choice {
                value: choice.value,
                label: choice.label,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// `value` as a setting's value, or `None` if it has none or is blank, which leaves the setting
/// out.
fn setting_value_from_proto(value: &proto::SettingValue) -> Option<settings::SettingValue> {
    settings::SettingValue::from_proto(value)
        .filter(|value| !matches!(value, settings::SettingValue::Text(text) if text.is_empty()))
}

/// The Connect error a client gets when the Plugins couldn't be listed, logged as
/// [`database_failed`] logs.
fn list_failed(err: ListError) -> ConnectError {
    match err {
        ListError::Read(err) => database_failed(err),
        err @ ListError::ReadInstalled(_) => {
            tracing::error!("{err}");
            ConnectError::internal("the Server failed")
        }
    }
}

/// The refusal of a list of Qualities for the Plugin `plugin` that breaks a list rule, naming the
/// field at fault.
fn invalid_qualities(err: qualities::Invalid, plugin: &PluginId) -> ConnectError {
    use RefusalReason as R;
    let message = err.to_string();
    let (reason, field) = match err {
        qualities::Invalid::Blank(index, part) => (
            R::REFUSAL_REASON_FIELD_REQUIRED,
            format!("qualities[{index}].{part}"),
        ),
        qualities::Invalid::Repeated(index, part) => (
            R::REFUSAL_REASON_QUALITY_REPEATED,
            format!("qualities[{index}].{part}"),
        ),
        qualities::Invalid::NotOneDefault => (
            R::REFUSAL_REASON_DEFAULT_QUALITY_REQUIRED,
            "qualities".to_owned(),
        ),
        qualities::Invalid::ProfileNotOffered(index, profile) => {
            let field = format!("qualities[{index}].{}", qualities::PROFILE);
            return refuse(R::REFUSAL_REASON_CHOICE_NOT_OFFERED, message)
                .field(&field)
                .param("pluginId", plugin)
                .param("settingKey", &qualities::QUALITY_PROFILE)
                .param("settingValue", &profile)
                .into();
        }
    };
    refuse(reason, message).field(&field).into()
}

#[cfg(test)]
mod tests {
    use nama_plugin_api::settings::{SettingField, SettingKind, SettingValue};

    use super::setting_to_proto;

    #[test]
    fn a_secrets_default_and_stored_value_are_never_sent() {
        let token = SettingField {
            key: "token".to_owned(),
            label: "Token".to_owned(),
            description: String::new(),
            kind: SettingKind::Secret,
            required: false,
            default: Some(SettingValue::Text("default-token".to_owned())),
        };
        let stored = SettingValue::Text("stored-token".to_owned());

        let shown = setting_to_proto(&token, Some(&stored));

        assert!(shown.default_value.is_unset(), "{shown:?}");
        assert!(shown.value.is_unset(), "{shown:?}");
        assert!(shown.secret_set);
    }
}
