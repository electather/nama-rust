//! A Built-in Plugin whose Jobs delete what the Server keeps that nobody will read again, each
//! through one Kernel function (ADR 0079).

use std::sync::Arc;

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    automation::{
        ExpiredSignInsAsked, ExpiredSignInsDeleted, ForgetHomeRowsAsked, HomeRowsForgotten, Kernel,
        Permission,
    },
    job::{Job, JobRunner, RunJob},
    settings::{SettingField, SettingKind, SettingValue, SettingsSchema},
};
use tracing::info;

/// The Job forgetting the Home rows of Users who stopped opening home.
const FORGET_HOME_ROWS: &str = "forget-home-rows";
/// The setting holding how many days without opening home a User's rows are kept.
const HOME_ROWS_DAYS: &str = "home_rows_days";
const DEFAULT_HOME_ROWS_DAYS: u32 = 30;
/// The Job deleting expired Devices and Invites.
const DELETE_EXPIRED_SIGN_INS: &str = "delete-expired-sign-ins";

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
            description: "Deletes what the Server keeps that nobody will read again.".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            settings: SettingsSchema {
                fields: vec![SettingField {
                    key: HOME_ROWS_DAYS.to_owned(),
                    label: "Days".to_owned(),
                    description: "How many days a User can go without opening home before their \
                                  Home rows and Featured suggestions are forgotten."
                        .to_owned(),
                    kind: SettingKind::Integer,
                    required: false,
                    default: Some(SettingValue::Integer(DEFAULT_HOME_ROWS_DAYS.into())),
                    min: Some(1),
                    max: Some(3650),
                }],
            },
            extension_points: vec![PointKey::provide::<dyn JobRunner>()],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: vec![
                Permission::PERMISSION_FORGET_HOME_ROWS,
                Permission::PERMISSION_DELETE_EXPIRED_SIGN_INS,
            ],
            jobs: vec![
                Job {
                    id: FORGET_HOME_ROWS.to_owned(),
                    name: "Forget unread Home rows".to_owned(),
                    description: "Deletes the Home rows and Featured suggestions of Users who \
                              haven't opened home in the days set. They get fresh ones when they \
                              come back."
                        .to_owned(),
                    schedule: Some("0 4 * * *".to_owned()),
                },
                Job {
                    id: DELETE_EXPIRED_SIGN_INS.to_owned(),
                    name: "Delete expired sign-ins".to_owned(),
                    description:
                        "Deletes the Devices and Invites that have expired, which no longer \
                              work."
                            .to_owned(),
                    schedule: Some("10 4 * * *".to_owned()),
                },
            ],
            ..Manifest::new(Self::ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let kernel = registrar
            .kernel()
            .ok_or("the Kernel handed Upkeep none of its functions")?;
        let home_rows_days = match registrar.settings().get(HOME_ROWS_DAYS) {
            Some(SettingValue::Integer(days)) => u32::try_from(*days)?,
            _ => DEFAULT_HOME_ROWS_DAYS,
        };
        let jobs = Jobs {
            kernel,
            home_rows_days,
        };
        registrar.provide::<dyn JobRunner>(Priority::DEFAULT, Arc::new(jobs));
        Ok(())
    }
}

/// Runs Upkeep's Jobs through the Kernel's functions.
struct Jobs {
    kernel: Arc<dyn Kernel>,
    home_rows_days: u32,
}

impl JobRunner for Jobs {
    fn run<'a>(&'a self, run: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            match run.job_id.as_str() {
                FORGET_HOME_ROWS => self.forget_home_rows().await,
                DELETE_EXPIRED_SIGN_INS => self.delete_expired_sign_ins().await,
                other => Err(format!("Upkeep has no Job {other}").into()),
            }
        })
    }
}

impl Jobs {
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
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
