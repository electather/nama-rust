import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { screen, within } from "@testing-library/react";

import { ExtensionPoint, PluginKind, PluginService, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";

/** TMDB's logo, the one Plugin here with one. */
export const tmdbLogo =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1" fill="#0d253f"/></svg>';

const text = (value: string) => ({ value: { case: "textValue" as const, value } });

/**
 * The Built-in Plugins of a fresh Server with a TMDB API key set, settings of every kind (log-events
 * missing its required Path), and the test Plugin, whose settings check offers choices, then an
 * Installed Plugin with a URL setting and one the Server refused. TMDB, the test Plugin and the
 * Installed Plugin name their Extension Points. Only TMDB has a logo.
 */
const plugins = [
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
    extensionPoints: [ExtensionPoint.METADATA_PROVIDER],
    logoSvg: tmdbLogo,
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
    id: "out-of-range.wasm",
    version: "",
    kind: PluginKind.INSTALLED,
    enabled: false,
    refusedReason: "it wants nama-api <0.1, and this Server has 0.1.0",
  },
];

/** What `ListPlugins` lists of one of `plugins`, and its `Plugin` holds. */
function summaryOf({
  id,
  name = "",
  description = "",
  version,
  kind,
  enabled,
  refusedReason = "",
  settingsMissing = false,
  extensionPoints = [],
  logoSvg = "",
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
    logoSvg,
  };
}

/** A fake Server that lists `plugins`, gets each by id, and answers other calls with `changes`. */
function pluginServer(changes: Partial<ServiceImpl<typeof PluginService>>) {
  return fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }, {}, {}, (router) => {
    router.service(PluginService, {
      listPlugins: () => ({ summaries: plugins.map((listed) => summaryOf(listed)) }),
      getPlugin: ({ id }) => {
        const found = plugins.find((listed) => listed.id === id);
        if (!found) throw new ConnectError(`there is no Plugin ${id}`, Code.NotFound);
        // Sending it drops what `Plugin` has no field for: those are the summary's.
        return { plugin: { ...found, summary: summaryOf(found) } };
      },
      ...changes,
    });
  });
}

/** Opens the Plugins list on a fake Server with `plugins`, answering other calls with `changes`. */
export function openPlugins(changes: Partial<ServiceImpl<typeof PluginService>> = {}) {
  openApp("/admin/plugins", pluginServer(changes));
}

/** Opens the Plugin `id`'s details page, as `openPlugins` does the list. */
export function openPlugin(id: string, changes: Partial<ServiceImpl<typeof PluginService>> = {}) {
  openApp(`/admin/plugins/${id}`, pluginServer(changes));
}

/** The Plugins list's row for the Plugin shown as `name`: its name, or its id without one. */
export async function row(name: string) {
  const listed = (await screen.findByRole("link", { name })).closest("li");
  if (listed === null) throw new Error(`${name} has no row`);
  return within(listed);
}

/** The details page of the Plugin shown as `name`, as `row` finds it, once it shows. */
export async function plugin(name: string) {
  await screen.findByRole("heading", { level: 1, name });
  return within(screen.getByRole("main"));
}

/** The input for the setting labelled `label` on the details page of the Plugin shown as `name`. */
export async function input(name: string, label: string) {
  return (await plugin(name)).getByLabelText<HTMLInputElement>(label);
}
