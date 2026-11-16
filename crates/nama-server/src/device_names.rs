//! Names a signing-in Device after its browser and OS, read from its `User-Agent`, for clients
//! that don't name themselves. The User can rename it in Settings.

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
pub(crate) fn from_user_agent(user_agent: Option<&str>) -> String {
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

#[cfg(test)]
#[path = "device_names_tests.rs"]
mod tests;
