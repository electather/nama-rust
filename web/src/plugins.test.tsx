import { Code, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginKind, PluginService, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

const text = (value: string) => ({ value: { case: "textValue" as const, value } });

/** The Built-in Plugins of a fresh Server with a TMDB API key set, and settings of every kind. */
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
];

function openPlugins(changes: Partial<ServiceImpl<typeof PluginService>> = {}) {
  openApp(
    "/admin/plugins",
    fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }, {}, {}, (router) => {
      router.service(PluginService, { listPlugins: () => ({ plugins }), ...changes });
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

test("a bool shows as on or off, a number as itself, and a missing required setting says so", async () => {
  openPlugins();

  expect(await setting("log-events", "Verbose")).toBe("On");
  expect(await setting("log-events", "Keep for days")).toBe("30 (default)");
  expect(await setting("log-events", "Path")).toBe("Not set (required)");
});

test("disabling an enabled Plugin asks the Server to", async () => {
  const disabled: string[] = [];
  openPlugins({
    disablePlugin: ({ id }) => {
      disabled.push(id);
      return {};
    },
  });
  fireEvent.click((await plugin("tmdb")).getByRole("button", { name: "Disable tmdb" }));

  await waitFor(() => {
    expect(disabled).toStrictEqual(["tmdb"]);
  });
});

test("enabling a disabled Plugin asks the Server to", async () => {
  const enabled: string[] = [];
  openPlugins({
    enablePlugin: ({ id }) => {
      enabled.push(id);
      return {};
    },
  });
  fireEvent.click((await plugin("log-events")).getByRole("button", { name: "Enable log-events" }));

  await waitFor(() => {
    expect(enabled).toStrictEqual(["log-events"]);
  });
});

test("disabling the only Plugin Users sign in with says why it can’t be", async () => {
  openPlugins({
    disablePlugin: () => {
      throw refused(
        Code.FailedPrecondition,
        "local-auth can't be disabled while no other enabled Plugin lets Users sign in",
        {
          reason: RefusalReason.LAST_AUTHENTICATOR,
          field: "id",
        },
      );
    },
  });
  const localAuth = await plugin("local-auth");
  fireEvent.click(localAuth.getByRole("button", { name: "Disable local-auth" }));

  expect((await localAuth.findByRole("alert")).textContent).toBe(
    "It can’t be disabled while no other enabled Plugin lets Users sign in.",
  );
});

test("enabling a Plugin missing a required setting names the setting", async () => {
  openPlugins({
    enablePlugin: () => {
      throw refused(Code.InvalidArgument, "the setting path is required", {
        reason: RefusalReason.FIELD_REQUIRED,
        field: "settings.path",
      });
    },
  });
  const logEvents = await plugin("log-events");
  fireEvent.click(logEvents.getByRole("button", { name: "Enable log-events" }));

  expect((await logEvents.findByRole("alert")).textContent).toBe(
    "It needs Path set before it can be enabled.",
  );
});
