//! How the Server crate's files are laid out. The rules are under "Structure" in
//! `crates/nama-server/AGENTS.md`; `nama-structure` checks them against the tables here.

use std::path::Path;

use nama_structure::{
    Allowed, Exception, Importers, MustFind, Structure, is_test_file, production_lines, rust_files,
};

#[test]
fn the_server_keeps_its_structure() {
    Structure {
        root: env!("CARGO_MANIFEST_DIR"),
        ceilings: CEILINGS,
        owned_tables: OWNED_TABLES,
        rows: ROWS,
        rows_name_areas_to_come: false,
        startup: STARTUP,
        allowed_upward: ALLOWED_UPWARD,
        surface_exceptions: SURFACE_EXCEPTIONS,
        confined_crates: &[],
        must_find: MustFind {
            nested_folder: "src/playback/choices",
            area_module: Some(("library", "contents")),
            crate_paths: 101,
            named_paths: 10,
        },
    }
    .check();
}

/// Today's files over 800 lines, sorted by path, each with the ceiling it stays at or under.
const CEILINGS: &[(&str, usize)] = &[
    ("tests/access.rs", 900),
    ("tests/api.rs", 900),
    ("tests/browse.rs", 1700),
    ("tests/delete_media.rs", 1300),
    ("tests/playback.rs", 2600),
    ("tests/plugin_service.rs", 1100),
    ("tests/plugins.rs", 900),
    ("tests/requests.rs", 4100),
    ("tests/sign_in.rs", 1400),
    ("tests/users.rs", 1200),
];

/// The tables a "One owner" rule in `AGENTS.md` names, each with its owner's path without `.rs`.
const OWNED_TABLES: &[(&str, &[&str])] = &[
    (
        "src/library/contents",
        &[
            "library_files",
            "library_items",
            "item_matches",
            "item_match_ids",
            "match_pins",
            "title_search",
            "title_search_vocab",
            "local_images",
            "nfo_files",
            "title_details",
            "title_roles",
            "episode_air_dates",
            "similar_titles",
            "box_sets",
        ],
    ),
    (
        "src/playback/tracks",
        &[
            "version_probes",
            "tracks",
            "chapters",
            "probe_failures",
            "segments",
        ],
    ),
    (
        "src/requests/qualities",
        &["qualities", "default_qualities"],
    ),
    ("src/requests", &["request_title_ids", "followed_show_ids"]),
    ("src/server_settings", &["server_settings"]),
    ("src/images", &["image_thumbhashes"]),
    ("src/library/custom_images", &["custom_images"]),
    ("src/playback/track_preferences", &["track_preferences"]),
    ("src/playback/autoplay", &["autoplay_off"]),
    (
        "src/playback/subtitle_appearance",
        &["subtitle_appearances"],
    ),
    ("src/playback/subtitle_offsets", &["subtitle_offsets"]),
    (
        "src/playback/sidecars",
        &["sidecar_subtitles", "downloaded_subtitles"],
    ),
    ("src/library/watch_states", &["watch_states"]),
    ("src/users/personal_settings", &["personal_settings"]),
    ("src/users/inbox", &["inbox"]),
    ("src/users/library_access", &["user_libraries"]),
    ("src/automation/store", &["plugin_store"]),
    ("src/watchlist", &["watchlist"]),
    ("src/playback/preview_frames", &["preview_frames"]),
    ("src/home_rows/refreshes", &["home_visits"]),
    ("src/link/server", &["hub_link", "hub_key_set"]),
];

/// The Server's modules in rows of who may call whom, highest row first (ADR 0051). A module may
/// call a lower row, or a module to its right in its own row; `STARTUP` calls any row. The calls
/// allowed against the order are in `ALLOWED_UPWARD`.
// `web` imports nothing from the crate. `plugin_settings` stays a single file: its folder holds only
// private helpers (`stored.rs`, `put_back.rs`, `running.rs`) and its one rule, "One owner for
// Plugin changes", stays in the crate's AGENTS.md.
const ROWS: &[&[&str]] = &[
    &["api", "automation", "status"],
    &["sync", "watch_sync", "browse", "scanner", "plugin_settings"],
    &["home_rows", "requests", "playback", "matching", "watchlist"],
    &["library"],
    &[
        "events",
        "connections",
        "users",
        "server_addresses",
        "certificate",
        "relay",
        "link",
    ],
    &["plugins"],
    &["backup"],
    &[
        "images",
        "server_settings",
        "ffmpeg",
        "web",
        "port_mapping",
        "tls",
        "diagnostics",
        "signed_urls",
        "db",
        "page",
        "config",
        "log_capture",
    ],
];

/// The modules that start the Server, which call any row.
const STARTUP: &[&str] = &["lib", "main"];

/// The calls against `ROWS` that are allowed anyway: who makes them, the leading segments of
/// the `crate::` path they name, and why. An entry no import uses fails.
const ALLOWED_UPWARD: &[Allowed] = &[
    (
        Importers::File("src/library/contents/decisions.rs"),
        &["requests", "make_available"],
        "ADR 0012: the Library makes Requests Available inside its own write",
    ),
    (
        Importers::File("src/library/contents/decisions.rs"),
        &["requests", "make_seasons_available"],
        "ADR 0012: the Library makes Requests Available inside its own write",
    ),
    (
        Importers::File("src/library/libraries.rs"),
        &["playback", "make_segments_due"],
        "ADR 0075: a Segment source added to a Library makes its Versions due inside its write",
    ),
    (
        Importers::File("src/library/contents/ids.rs"),
        &["playback", "title_ids_changed"],
        "ADR 0075: a Match whose ids change makes its Version due inside its write",
    ),
    (
        Importers::File("src/plugin_settings/leftovers.rs"),
        &["automation", "delete_plugins_store_entries"],
        "the Plugin leftovers owner deletes a removed Plugin's Store entries through the Store's \
         owner, in Upkeep's one write",
    ),
    (
        Importers::File("src/playback/tracks/segments_tests.rs"),
        &["plugin_settings", "PluginSettings"],
        "the Segments tests enable a Plugin as an Admin does, to check it makes Versions due",
    ),
];

/// The modules neighbours reach inside an area anyway, each with why re-exporting it would be
/// wrong. An entry no import uses fails.
const SURFACE_EXCEPTIONS: &[Exception] = &[(
    "ffmpeg",
    "fake",
    "the crate's Fake programs rule sends every test's fake program through ffmpeg::fake::script, \
     and neighbours' tests share its two-GPU VAAPI fake",
)];

// ponytail: a text scan; a call split across lines or reached through a `use` alias is not seen.
#[test]
fn production_code_builds_users_only_at_startup() {
    let problems: Vec<String> = rust_files(Path::new(env!("CARGO_MANIFEST_DIR")))
        .iter()
        .filter(|file| file.path.starts_with("src/") && !is_test_file(&file.path))
        .filter(|file| !matches!(file.path.as_str(), "src/lib.rs" | "src/main.rs"))
        .flat_map(|file| {
            production_lines(&file.text)
                .filter(|(_, line)| line.contains("Users::new("))
                .map(|(line, _)| {
                    format!(
                        "{}:{line}: builds a second Users: take the Server's one (src/users/AGENTS.md)",
                        file.path
                    )
                })
        })
        .collect();
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

/// The files besides `src/link.rs` that may name the Plugin that links the Server, each with why.
const MAY_NAME_THE_SERVER_LINKER: &[(&str, &str)] = &[(
    "src/api/plugins/points.rs",
    "the Plugins page lists every Extension Point a Plugin may serve",
)];

#[test]
fn only_the_link_module_calls_the_plugin_that_links_the_server() {
    let problems: Vec<String> = rust_files(Path::new(env!("CARGO_MANIFEST_DIR")))
        .iter()
        .filter(|file| file.path.starts_with("src/") && file.path != "src/link.rs")
        .filter(|file| {
            !MAY_NAME_THE_SERVER_LINKER
                .iter()
                .any(|(path, _)| file.path == *path)
        })
        .filter(|file| file.text.contains("ServerLinker"))
        .map(|file| {
            format!(
                "{}: names the ServerLinker: ask through `Linking` in src/link.rs (AGENTS.md)",
                file.path
            )
        })
        .collect();
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
