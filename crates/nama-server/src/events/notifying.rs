//! Handing a notification to a Notifier for its channel and for each User the Kernel picks to
//! tell, through their Plugin Connection or Personal settings (ADR 0021, ADR 0046).

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use futures_util::{StreamExt, future, stream};
use nama_plugin_api::{
    PluginError, PluginId,
    notifier::{DELIVER_TIMEOUT, Notification, Notifier, Recipient, RecipientGone},
};
use nama_proto::nama::v1::{Event, NotificationKind};
use tracing::{error, info};

use super::users_to_tell;
use crate::{
    connections,
    db::Database,
    plugins::{Failure, ProviderError, without_secrets},
    users::{
        User, delete_personal_settings, every_notification_turned_off, personal_settings_recipients,
    },
};

/// How many recipients one Notifier delivers a notification to at once. An Installed Plugin's
/// delivery holds a blocking thread, so a Request with many Users to tell mustn't take one each.
const DELIVERING_AT_ONCE: usize = 4;

/// Hands `told`'s notification to `plugin`'s `notifier` for its channel, and for the Plugin
/// Connection to `plugin` and the Personal settings for it of each User told about its event,
/// [`DELIVERING_AT_ONCE`] at a time. A User with neither is skipped, and one the Notifier finds
/// gone is disconnected (ADR 0021), or, for Personal settings, has them deleted (ADR 0046).
///
/// # Errors
///
/// Each recipient that failed, with the Notifier's error, that User's secret Personal settings
/// blanked. Reading or deleting Personal settings or Plugin Connections failing is logged,
/// and the others still told.
pub(super) async fn deliver(
    db: Arc<Database>,
    plugin: PluginId,
    notifier: Arc<dyn Notifier>,
    told: Arc<(Event, Notification)>,
) -> Result<(), Undelivered> {
    let (event, notification) = &*told;
    let mut recipients = vec![(None, Recipient::Channel)];
    let turned_off = match every_notification_turned_off(&db).await {
        Ok(turned_off) => turned_off,
        // Telling someone a kind they turned off beats missing one they want.
        Err(err) => {
            error!(%plugin, "could not read who turned notifications off, so told all: {err}");
            Vec::new()
        }
    };
    match connections::connected(&db, &plugin).await {
        Ok(connected) => recipients.extend(connections_to_tell(event, connected, &turned_off)),
        Err(err) => error!(%plugin, "could not read whom to message about a Request: {err}"),
    }
    let mut secrets = HashMap::new();
    match personal_settings_recipients(&db, &plugin).await {
        Ok(personal) => {
            recipients.extend(personal_to_tell(event, personal, &turned_off, &mut secrets));
        }
        Err(err) => error!(%plugin, "could not read whose Personal settings to use: {err}"),
    }
    let deliveries = recipients.into_iter().map(|(user, recipient)| {
        let (db, plugin, notifier, secrets) = (&db, &plugin, &notifier, &secrets);
        async move {
            let delivered = deliver_to(notifier.as_ref(), notification, &recipient).await;
            match (delivered, user, recipient) {
                (Ok(()), ..) => None,
                (Err(error), Some(user), Recipient::Connection(connection))
                    if error.is::<RecipientGone>() =>
                {
                    disconnect(db, user, plugin, connection).await;
                    None
                }
                (Err(error), Some(user), Recipient::User(_)) if error.is::<RecipientGone>() => {
                    forget_personal_settings(db, user, plugin).await;
                    None
                }
                (Err(error), user, _) => {
                    let theirs = user.and_then(|user| secrets.get(&user));
                    Some((
                        user,
                        without_secrets(error.to_string(), theirs.map_or(&[], Vec::as_slice)),
                    ))
                }
            }
        }
    });
    let failed: Vec<_> = stream::iter(deliveries)
        .buffer_unordered(DELIVERING_AT_ONCE)
        .filter_map(future::ready)
        .collect()
        .await;
    if failed.is_empty() {
        Ok(())
    } else {
        Err(Undelivered(failed))
    }
}

/// The recipients a Notifier didn't reach, each with its error: a User by id, or the channel.
pub(super) struct Undelivered(Vec<(Option<i64>, String)>);

impl ProviderError<()> for Undelivered {
    fn failure(self) -> Failure {
        let each = self.0.into_iter().map(|(user, error)| match user {
            // By id, so that no log names a chat id.
            Some(user) => format!("to User {user}: {error}"),
            None => format!("to the channel: {error}"),
        });
        Failure {
            kind: (),
            message: each.collect::<Vec<_>>().join("; "),
        }
    }
}

/// The Plugin Connection of each of `connected` that [`users_to_tell`] picks for `event`, with
/// the id of the User whose it is.
fn connections_to_tell(
    event: &Event,
    connected: Vec<(User, String)>,
    turned_off: &[(i64, NotificationKind)],
) -> Vec<(Option<i64>, Recipient)> {
    let (users, connections): (Vec<_>, Vec<_>) = connected.into_iter().unzip();
    let told = users_to_tell(event, &users, turned_off);
    users
        .iter()
        .zip(connections)
        .filter(|(user, _)| told.contains(&user.id))
        .map(|(user, connection)| (Some(user.id), Recipient::Connection(connection)))
        .collect()
}

/// The Personal settings of each of `personal` that [`users_to_tell`] picks for `event`, with the
/// id of the User whose they are. Each picked User's secret values go in `secrets`.
fn personal_to_tell(
    event: &Event,
    personal: Vec<(User, BTreeMap<String, String>, Vec<String>)>,
    turned_off: &[(i64, NotificationKind)],
    secrets: &mut HashMap<i64, Vec<String>>,
) -> Vec<(Option<i64>, Recipient)> {
    let users: Vec<User> = personal.iter().map(|(user, ..)| user.clone()).collect();
    let told = users_to_tell(event, &users, turned_off);
    personal
        .into_iter()
        .filter(|(user, ..)| told.contains(&user.id))
        .map(|(user, values, theirs)| {
            secrets.insert(user.id, theirs);
            (Some(user.id), Recipient::User(values))
        })
        .collect()
}

/// Deletes `user`'s Plugin Connection to `plugin`, `connection`, which its Notifier found gone.
async fn disconnect(db: &Database, user: i64, plugin: &PluginId, connection: String) {
    match connections::forget(db, user, plugin, connection).await {
        Ok(()) => info!(%plugin, user, "disconnected a User the service says is gone"),
        Err(err) => error!(%plugin, user, "could not disconnect a User who is gone: {err}"),
    }
}

/// Deletes `user`'s Personal settings for `plugin`, which its Notifier found reach nobody.
async fn forget_personal_settings(db: &Database, user: i64, plugin: &PluginId) {
    match delete_personal_settings(db, user, plugin).await {
        Ok(()) => info!(%plugin, user, "deleted Personal settings the service says reach nobody"),
        Err(err) => {
            error!(%plugin, user, "could not delete Personal settings that reach nobody: {err}");
        }
    }
}

/// Hands `notification` to `notifier` for `recipient`, giving up after [`DELIVER_TIMEOUT`].
async fn deliver_to(
    notifier: &dyn Notifier,
    notification: &Notification,
    recipient: &Recipient,
) -> Result<(), PluginError> {
    let delivered = notifier.deliver(notification, recipient);
    tokio::time::timeout(DELIVER_TIMEOUT, delivered)
        .await
        .unwrap_or_else(|elapsed| {
            Err(format!("timed out after {DELIVER_TIMEOUT:?} ({elapsed})").into())
        })
}
