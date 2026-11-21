import { ExtensionPoint, PluginKind, SettingKind } from "@/gen/nama/v1/plugin_pb";

/** Where TMDB's logo is, the one Plugin here with one. */
export const tmdbLogo = "/media/plugin-logos/tmdb/v1";

const text = (value: string) => ({ value: { case: "textValue" as const, value } });
const texts = (...values: string[]) => ({ value: { case: "texts" as const, value: { values } } });

/**
 * The Built-in Plugins of a fresh Server with a TMDB API key set, settings of every kind (log-events
 * missing its required Path), and the test Plugin, whose settings check offers choices and whose
 * Ratings hold a pick it no longer offers, then an Installed Plugin with a URL setting, one with a
 * Libraries setting holding Movies and a Library since deleted, and one the Server refused. TMDB,
 * the test Plugin and the first Installed Plugin name their Extension Points. Only TMDB has a logo.
 */
export const plugins = [
  { id: "local-auth", version: "0.1.0", kind: PluginKind.BUILT_IN, enabled: true },
  {
    id: "log-events",
    version: "0.1.0",
    kind: PluginKind.BUILT_IN,
    enabled: false,
    settingsMissing: true,
    settings: [
      {
        key: "verbose",
        label: "Verbose",
        description: "",
        kind: SettingKind.BOOL,
        defaultValue: { value: { case: "boolValue" as const, value: false } },
        value: { value: { case: "boolValue" as const, value: true } },
      },
      {
        key: "keep_days",
        label: "Keep for days",
        description: "",
        kind: SettingKind.INTEGER,
        defaultValue: { value: { case: "integerValue" as const, value: 30n } },
      },
      { key: "path", label: "Path", description: "", kind: SettingKind.TEXT, required: true },
    ],
  },
  {
    id: "tmdb",
    name: "TMDB",
    description: "Finds Movies and Shows on TMDB.",
    version: "0.1.0",
    kind: PluginKind.BUILT_IN,
    enabled: true,
    extensionPoints: [ExtensionPoint.TITLE_SOURCE],
    logoUrl: tmdbLogo,
    settings: [
      {
        key: "api_key",
        label: "API key",
        description: "Your TMDB API key.",
        kind: SettingKind.SECRET,
        required: true,
        secretSet: true,
      },
      {
        key: "language",
        label: "Language",
        description: "The language titles are fetched in.",
        kind: SettingKind.TEXT,
        defaultValue: text("en-US"),
        value: text("de-DE"),
      },
      {
        key: "region",
        label: "Region",
        description: "Where release dates are for.",
        kind: SettingKind.TEXT,
        defaultValue: text("US"),
      },
      { key: "token", label: "Token", description: "", kind: SettingKind.SECRET },
    ],
  },
  {
    id: "test-choices",
    version: "0.0.0",
    kind: PluginKind.BUILT_IN,
    enabled: true,
    extensionPoints: [ExtensionPoint.MOVIE_FULFILLER, ExtensionPoint.SHOW_FULFILLER],
    checksSettings: true,
    settings: [
      {
        key: "api_key",
        label: "API key",
        description: "",
        kind: SettingKind.SECRET,
        required: true,
        secretSet: true,
      },
      {
        key: "profile",
        label: "Profile",
        description: "One of the profiles the check offers.",
        kind: SettingKind.CHOICE,
        required: true,
        value: text("1"),
      },
      {
        key: "ratings",
        label: "Ratings",
        description: "The ratings to keep.",
        kind: SettingKind.CHOICES,
        choices: [
          { value: "g", label: "G" },
          { value: "pg", label: "PG" },
          { value: "r", label: "R" },
        ],
        value: texts("pg", "nc-17"),
      },
    ],
  },
  {
    id: "test-plugin",
    version: "0.1.0",
    kind: PluginKind.INSTALLED,
    enabled: false,
    extensionPoints: [ExtensionPoint.NOTIFIER, ExtensionPoint.EVENTS_OBSERVER],
    settings: [
      {
        key: "url",
        label: "Sonarr URL",
        description: "Where Sonarr is.",
        kind: SettingKind.URL,
        value: text("http://sonarr.lan:8989"),
      },
    ],
  },
  {
    id: "cleanup",
    version: "0.1.0",
    kind: PluginKind.INSTALLED,
    enabled: false,
    extensionPoints: [ExtensionPoint.AUTOMATION],
    settings: [
      {
        key: "days",
        label: "Days",
        description: "",
        kind: SettingKind.INTEGER,
        minValue: 0n,
        defaultValue: { value: { case: "integerValue" as const, value: 30n } },
      },
      {
        key: "libraries",
        label: "Libraries to clean",
        description: "Where watched titles are deleted.",
        kind: SettingKind.LIBRARIES,
        value: { value: { case: "libraryIds" as const, value: { ids: [1n, 9n] } } },
      },
    ],
  },
  {
    id: "out-of-range.wasm",
    version: "",
    kind: PluginKind.INSTALLED,
    enabled: false,
    refusedReason: "it wants Plugin contract <0.1, and this Server has 0.1.0",
  },
];

/** What `ListPlugins` lists of one of `plugins`, and its `Plugin` holds. */
export function summaryOf({
  id,
  name = "",
  description = "",
  version,
  kind,
  enabled,
  refusedReason = "",
  settingsMissing = false,
  extensionPoints = [],
  logoUrl = "",
}: (typeof plugins)[number]) {
  return {
    id,
    name,
    description,
    version,
    kind,
    enabled,
    refusedReason,
    settingsMissing,
    extensionPoints,
    logoUrl,
  };
}

/** The Libraries on the fake Server: Movies, Shows and Anime, but no longer Library 9. */
export const libraries = [
  { id: 1n, name: "Movies" },
  { id: 2n, name: "Shows" },
  { id: 3n, name: "Anime" },
];
