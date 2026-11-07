import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginKind, PluginService, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";

afterEach(cleanup);

const text = (value: string) => ({ value: { case: "textValue" as const, value } });

/** The Built-in Plugins of a fresh Server with a TMDB API key set. */
const plugins = [
  { id: "local-auth", version: "0.1.0", kind: PluginKind.BUILT_IN, enabled: true },
  { id: "log-events", version: "0.1.0", kind: PluginKind.BUILT_IN, enabled: false },
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
];

function openPlugins() {
  openApp(
    "/admin/plugins",
    fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }, {}, {}, (router) => {
      router.service(PluginService, { listPlugins: () => ({ plugins }) });
    }),
  );
}

async function plugin(id: string) {
  return within(await screen.findByRole("region", { name: id }));
}

/** The value the Plugin `id` shows for its setting labelled `label`. */
async function setting(id: string, label: string) {
  const term = (await plugin(id)).getByText(label);
  return term.closest("div")?.querySelector("dd")?.textContent;
}

test("the Plugins page lists every Plugin with whether it is enabled", async () => {
  openPlugins();

  expect((await plugin("local-auth")).getByText("Enabled")).toBeDefined();
  expect((await plugin("log-events")).getByText("Disabled")).toBeDefined();
  expect((await plugin("tmdb")).getByText("Enabled")).toBeDefined();
});

test("a Plugin's settings show their values, defaults and whether a secret is set", async () => {
  openPlugins();

  expect(await setting("tmdb", "API key")).toBe("Set");
  expect(await setting("tmdb", "Language")).toBe("de-DE");
  expect(await setting("tmdb", "Region")).toBe("US (default)");
  expect(await setting("tmdb", "Token")).toBe("Not set");
  expect((await plugin("tmdb")).getByText("Your TMDB API key.")).toBeDefined();
});
