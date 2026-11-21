//! A Built-in Plugin whose Jobs back the Server up, delete what it keeps that nobody will read
//! again, and fetch Matched titles' metadata again, each through one Kernel function (ADR 0079).

use std::sync::Arc;

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    automation::{
        BackUpAsked, BackedUp, ExpiredSignInsAsked, ExpiredSignInsDeleted, ForgetGonePluginsAsked,
        ForgetHomeRowsAsked, GonePluginsForgotten, HomeRowsForgotten, Kernel, MetadataRefreshed,
        Permission, RefreshMetadataAsked,
    },
    job::{Job, JobRunner, RunJob},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use tracing::info;

/// The Job writing a Backup of the Server.
const BACK_UP: &str = "back-up";
/// The Job forgetting the Home rows of Users who stopped opening home.
const FORGET_HOME_ROWS: &str = "forget-home-rows";
/// The setting holding how many days without opening home a User's rows are kept.
const HOME_ROWS_DAYS: &str = "home_rows_days";
const DEFAULT_HOME_ROWS_DAYS: u32 = 30;
/// The Job deleting expired Devices and Invites.
const DELETE_EXPIRED_SIGN_INS: &str = "delete-expired-sign-ins";
/// The Job deleting what Plugins the Server no longer has left behind.
const FORGET_GONE_PLUGINS: &str = "forget-gone-plugins";
/// The setting holding how many days what a removed Plugin left behind is kept.
const GONE_PLUGINS_DAYS: &str = "gone_plugins_days";
const DEFAULT_GONE_PLUGINS_DAYS: u32 = 30;
/// The Job fetching every Matched title's metadata again.
const REFRESH_METADATA: &str = "refresh-metadata";

/// The Plugin.
#[derive(Debug, Default)]
pub struct Upkeep;

impl Upkeep {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("upkeep");
}

impl Plugin for Upkeep {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: "Upkeep".to_owned(),
            description: "Backs the Server up, deletes what it keeps that nobody will read again, \
                          and keeps Matched titles' metadata current."
                .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            settings: SettingsSchema {
                fields: vec![
                    days_field(
                        HOME_ROWS_DAYS,
                        "Days",
                        "How many days a User can go without opening home before their Home rows \
                         and Featured suggestions are forgotten.",
                        DEFAULT_HOME_ROWS_DAYS,
                    ),
                    days_field(
                        GONE_PLUGINS_DAYS,
                        "Days to keep a removed Plugin's data",
                        "How many days after the Server stops having a Plugin its settings, \
                         Plugin Store entries, Personal settings and Plugin Connections are \
                         deleted. Reinstalling it sooner keeps them.",
                        DEFAULT_GONE_PLUGINS_DAYS,
                    ),
                ],
            },
            extension_points: vec![PointKey::provide::<dyn JobRunner>()],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: vec![
                Permission::PERMISSION_FORGET_HOME_ROWS,
                Permission::PERMISSION_FORGET_GONE_PLUGINS,
                Permission::PERMISSION_DELETE_EXPIRED_SIGN_INS,
                Permission::PERMISSION_REFRESH_METADATA,
                Permission::PERMISSION_BACK_UP,
            ],
            jobs: jobs(),
            ..Manifest::new(Self::ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let kernel = registrar
            .kernel()
            .ok_or("the Kernel handed Upkeep none of its functions")?;
        let settings = registrar.settings();
        let jobs = Jobs {
            home_rows_days: days(settings, HOME_ROWS_DAYS, DEFAULT_HOME_ROWS_DAYS)?,
            gone_plugins_days: days(settings, GONE_PLUGINS_DAYS, DEFAULT_GONE_PLUGINS_DAYS)?,
            kernel,
        };
        registrar.provide::<dyn JobRunner>(Priority::DEFAULT, Arc::new(jobs));
        Ok(())
    }
}

/// The Jobs Upkeep declares, each with its default schedule.
fn jobs() -> Vec<Job> {
    vec![
        Job {
            id: BACK_UP.to_owned(),
            name: "Back up".to_owned(),
            description: "Writes a Backup of the Server into the Backup folder, then \
                          deletes the oldest it wrote beyond the count kept, both set on \
                          the Backups page. A Backup that fails deletes none."
                .to_owned(),
            // Before the 04:00 Jobs, so the Backup holds what they'd delete.
            schedule: Some("0 3 * * *".to_owned()),
        },
        Job {
            id: FORGET_HOME_ROWS.to_owned(),
            name: "Forget unread Home rows".to_owned(),
            description: "Deletes the Home rows and Featured suggestions of Users who \
                          haven't opened home in the days set. They get fresh ones when \
                          they come back."
                .to_owned(),
            schedule: Some("0 4 * * *".to_owned()),
        },
        Job {
            id: DELETE_EXPIRED_SIGN_INS.to_owned(),
            name: "Delete expired sign-ins".to_owned(),
            description: "Deletes the Devices and Invites that have expired, which no \
                          longer work."
                .to_owned(),
            schedule: Some("10 4 * * *".to_owned()),
        },
        Job {
            id: FORGET_GONE_PLUGINS.to_owned(),
            name: "Forget removed Plugins".to_owned(),
            description: "Deletes the settings, Plugin Store entries, Personal settings \
                          and Plugin Connections of each Plugin the Server hasn't had for \
                          the days set. A disabled Plugin is kept."
                .to_owned(),
            // After the daily Jobs, so they don't all run at once.
            schedule: Some("20 4 * * 0".to_owned()),
        },
        Job {
            id: REFRESH_METADATA.to_owned(),
            name: "Refresh metadata".to_owned(),
            description: "Fetches every Matched title again from its title source, so \
                          Scores, Roles, Trailers, Similar titles and new Episodes stay \
                          current. It never changes which title an Item is Matched to, \
                          and a title it can't fetch keeps what it had until the next run."
                .to_owned(),
            schedule: Some("30 4 * * 0".to_owned()),
        },
    ]
}

/// A setting of how many days something is kept, from 1 day to 10 years.
fn days_field(key: &str, label: &str, description: &str, default: u32) -> SettingField {
    SettingField {
        key: key.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: SettingKind::Integer,
        required: false,
        default: Some(SettingValue::Integer(default.into())),
        min: Some(1),
        max: Some(3650),
    }
}

/// The days setting `key` holds, or `default` if it holds none.
fn days(settings: &Settings, key: &str, default: u32) -> Result<u32, PluginError> {
    match settings.get(key) {
        Some(SettingValue::Integer(days)) => Ok(u32::try_from(*days)?),
        _ => Ok(default),
    }
}

/// Runs Upkeep's Jobs through the Kernel's functions.
struct Jobs {
    kernel: Arc<dyn Kernel>,
    home_rows_days: u32,
    gone_plugins_days: u32,
}

impl JobRunner for Jobs {
    fn run<'a>(&'a self, run: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            match run.job_id.as_str() {
                BACK_UP => self.back_up().await,
                FORGET_HOME_ROWS => self.forget_home_rows().await,
                DELETE_EXPIRED_SIGN_INS => self.delete_expired_sign_ins().await,
                FORGET_GONE_PLUGINS => self.forget_gone_plugins().await,
                REFRESH_METADATA => self.refresh_metadata().await,
                other => Err(format!("Upkeep has no Job {other}").into()),
            }
        })
    }
}

impl Jobs {
    async fn back_up(&self) -> Result<(), PluginError> {
        let asked = BackUpAsked::default().encode_to_vec();
        let answer = self.kernel.call("back_up", asked).await?;
        let backed_up = BackedUp::decode_from_slice(&answer)?;
        info!(
            name = backed_up.name,
            bytes = backed_up.size_bytes,
            "wrote a Backup"
        );
        Ok(())
    }

    async fn forget_home_rows(&self) -> Result<(), PluginError> {
        let asked = ForgetHomeRowsAsked {
            days: self.home_rows_days,
            ..ForgetHomeRowsAsked::default()
        };
        let answer = (self.kernel)
            .call("forget_home_rows", asked.encode_to_vec())
            .await?;
        let forgotten = HomeRowsForgotten::decode_from_slice(&answer)?;
        info!(
            users = forgotten.users,
            "forgot the Home rows of Users who stopped opening home"
        );
        Ok(())
    }

    async fn delete_expired_sign_ins(&self) -> Result<(), PluginError> {
        let asked = ExpiredSignInsAsked::default().encode_to_vec();
        let answer = (self.kernel).call("delete_expired_sign_ins", asked).await?;
        let deleted = ExpiredSignInsDeleted::decode_from_slice(&answer)?;
        info!(
            devices = deleted.devices,
            invites = deleted.invites,
            "deleted expired Devices and Invites"
        );
        Ok(())
    }

    async fn forget_gone_plugins(&self) -> Result<(), PluginError> {
        let asked = ForgetGonePluginsAsked {
            days: self.gone_plugins_days,
            ..ForgetGonePluginsAsked::default()
        };
        let answer = (self.kernel)
            .call("forget_gone_plugins", asked.encode_to_vec())
            .await?;
        let forgotten = GonePluginsForgotten::decode_from_slice(&answer)?;
        info!(
            plugins = ?forgotten.plugin_ids,
            "forgot what Plugins the Server no longer has left behind"
        );
        Ok(())
    }

    async fn refresh_metadata(&self) -> Result<(), PluginError> {
        let asked = RefreshMetadataAsked::default().encode_to_vec();
        let answer = self.kernel.call("refresh_metadata", asked).await?;
        let refreshed = MetadataRefreshed::decode_from_slice(&answer)?;
        info!(
            titles = refreshed.titles,
            unavailable = ?refreshed.unavailable_plugin_ids,
            "refreshed the metadata of Matched titles"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
