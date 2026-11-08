import type { ServiceImpl } from "@connectrpc/connect";
import { screen, within } from "@testing-library/react";

import { PluginKind, PluginService, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";

const text = (value: string) => ({ value: { case: "textValue" as const, value } });

/**
 * The Built-in Plugins of a fresh Server with a TMDB API key set, settings of every kind, and the
 * test Plugin, whose settings check offers choices.
 */
const plugins = [
  { id: "local-auth", version: "0.1.0", kind: PluginKind.BUILT_IN, enabled: true },
  {
    id: "log-events",
    version: "0.1.0",
    kind: PluginKind.BUILT_IN,
    enabled: false,
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
    version: "0.1.0",
    kind: PluginKind.BUILT_IN,
    enabled: true,
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
];

/** Opens the Plugins page on a fake Server that lists `plugins` and answers other calls with `changes`. */
export function openPlugins(changes: Partial<ServiceImpl<typeof PluginService>> = {}) {
  openApp(
    "/admin/plugins",
    fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }, {}, {}, (router) => {
      router.service(PluginService, { listPlugins: () => ({ plugins }), ...changes });
    }),
  );
}

/** The section of the Plugins page for the Plugin `id`. */
export async function plugin(id: string) {
  return within(await screen.findByRole("region", { name: id }));
}

/** The input for the setting labelled `label` of the Plugin `id`. */
export async function input(id: string, label: string) {
  return (await plugin(id)).getByLabelText<HTMLInputElement>(label);
}
