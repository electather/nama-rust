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
mod tests {
    use super::from_user_agent;

    #[test]
    fn names_common_browsers_and_systems() {
        let cases = [
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
                "Chrome on macOS",
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
                "Edge on Windows",
            ),
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
                "Safari on iPhone",
            ),
            (
                "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
                "Chrome on Android",
            ),
            (
                "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0",
                "Firefox on Linux",
            ),
            ("curl/8.7.1", "Unknown device"),
        ];
        for (user_agent, name) in cases {
            assert_eq!(from_user_agent(Some(user_agent)), name, "{user_agent}");
        }
        assert_eq!(from_user_agent(None), "Unknown device");
    }
}
