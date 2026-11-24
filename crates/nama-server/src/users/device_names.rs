//! Names a signing-in Device after its browser and OS, read from its `User-Agent`, for clients
//! that don't name themselves, and tells Plugins what kind of Device it is. The User can rename it
//! in Settings.

use nama_plugin_api::contract::DeviceKind;

use super::Users;

/// Browsers, matched in order: Edge and Opera also claim to be Chrome, and Chrome claims Safari.
const BROWSERS: &[(&str, &str)] = &[
    ("Edg", "Edge"),
    ("OPR/", "Opera"),
    ("Firefox/", "Firefox"),
    ("FxiOS", "Firefox"),
    ("CriOS", "Chrome"),
    ("Chrome/", "Chrome"),
    ("Safari/", "Safari"),
];

/// Operating systems, matched in order: iOS says "like Mac OS X", and Android and `ChromeOS` say
/// Linux.
const SYSTEMS: &[(&str, &str)] = &[
    ("iPhone", "iPhone"),
    ("iPad", "iPad"),
    ("Android", "Android"),
    ("CrOS", "ChromeOS"),
    ("Windows", "Windows"),
    ("Macintosh", "macOS"),
    ("Linux", "Linux"),
];

/// A Device name for a client sending `user_agent`, such as "Chrome on macOS".
// ponytail: substring matching covers the common browsers; reach for a UA parser crate if Devices
// from other clients need better names.
pub(super) fn from_user_agent(user_agent: Option<&str>) -> String {
    let user_agent = user_agent.unwrap_or_default();
    let find = |table: &[(&str, &'static str)]| {
        table
            .iter()
            .find(|(needle, _)| user_agent.contains(needle))
            .map(|&(_, name)| name)
    };
    match (find(BROWSERS), find(SYSTEMS)) {
        (Some(browser), Some(system)) => format!("{browser} on {system}"),
        (Some(name), None) | (None, Some(name)) => name.to_owned(),
        (None, None) => "Unknown device".to_owned(),
    }
}

/// Clues to a Device's kind in its `User-Agent`, matched in order: TVs and Android tablets also
/// say Android, and Android phones say "Mobile", which tablets don't.
const KINDS: &[(&str, DeviceKind)] = &[
    ("SmartTV", DeviceKind::DEVICE_KIND_TV),
    ("SMART-TV", DeviceKind::DEVICE_KIND_TV),
    ("Tizen", DeviceKind::DEVICE_KIND_TV),
    ("Web0S", DeviceKind::DEVICE_KIND_TV),
    ("CrKey", DeviceKind::DEVICE_KIND_TV),
    ("GoogleTV", DeviceKind::DEVICE_KIND_TV),
    ("Android TV", DeviceKind::DEVICE_KIND_TV),
    ("AFT", DeviceKind::DEVICE_KIND_TV),
    ("AppleTV", DeviceKind::DEVICE_KIND_TV),
    ("Xbox", DeviceKind::DEVICE_KIND_TV),
    ("PlayStation", DeviceKind::DEVICE_KIND_TV),
    ("iPhone", DeviceKind::DEVICE_KIND_PHONE),
    ("iPad", DeviceKind::DEVICE_KIND_TABLET),
    ("Mobile", DeviceKind::DEVICE_KIND_PHONE),
    ("Android", DeviceKind::DEVICE_KIND_TABLET),
    ("Windows", DeviceKind::DEVICE_KIND_COMPUTER),
    ("Macintosh", DeviceKind::DEVICE_KIND_COMPUTER),
    ("CrOS", DeviceKind::DEVICE_KIND_COMPUTER),
    ("Linux", DeviceKind::DEVICE_KIND_COMPUTER),
];

/// What kind of Device sends `user_agent`.
// ponytail: substring matching, as for names; an iPad asking for desktop sites says Macintosh, so
// it reads as a computer.
fn kind_from_user_agent(user_agent: Option<&str>) -> DeviceKind {
    let user_agent = user_agent.unwrap_or_default();
    (KINDS.iter())
        .find(|(clue, _)| user_agent.contains(clue))
        .map_or(DeviceKind::DEVICE_KIND_UNSPECIFIED, |&(_, kind)| kind)
}

impl Users {
    /// Each of the Devices `devices` the Server still has that acts as a User, by id, with that
    /// User's id and its kind, in no order.
    pub(crate) async fn device_kinds(
        &self,
        devices: &[i64],
    ) -> Result<Vec<(i64, i64, DeviceKind)>, sqlx::Error> {
        let asked = serde_json::Value::from(devices).to_string();
        let rows = sqlx::query!(
            r#"SELECT id AS "id!", user_id AS "user_id!", user_agent FROM devices
               WHERE id IN (SELECT value FROM json_each(?)) AND user_id IS NOT NULL"#,
            asked,
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.id,
                    row.user_id,
                    kind_from_user_agent(row.user_agent.as_deref()),
                )
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "device_names_tests.rs"]
mod tests;
