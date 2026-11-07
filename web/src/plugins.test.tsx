import { Code, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  PluginKind,
  PluginService,
  SettingKind,
  type UpdatePluginSettingsRequest,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

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

/** Opens the Plugins page on a fake Server that answers `UpdatePluginSettings` with `update`. */
function openPlugins(
  update: ServiceImpl<typeof PluginService>["updatePluginSettings"] = () => ({}),
) {
  openApp(
    "/admin/plugins",
    fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }, {}, {}, (router) => {
      router.service(PluginService, {
        listPlugins: () => ({ plugins }),
        updatePluginSettings: update,
      });
    }),
  );
}

async function plugin(id: string) {
  return within(await screen.findByRole("region", { name: id }));
}

/** The input for the setting labelled `label` of the Plugin `id`. */
async function input(id: string, label: string) {
  return (await plugin(id)).getByLabelText<HTMLInputElement>(label);
}

/**
 * Opens the Plugins page to save the Plugin `id`'s settings: `submit` saves them, and `sent` is
 * the request the Server got.
 */
function openToSave(id: string) {
  const sent = new Promise<UpdatePluginSettingsRequest>((resolve) => {
    openPlugins((request) => {
      resolve(request);
      return { plugin: { id, enabled: true } };
    });
  });
  return {
    sent,
    submit: async () => fireEvent.click((await plugin(id)).getByRole("button", { name: "Save" })),
  };
}

test("the Plugins page lists every Plugin with whether it is enabled", async () => {
  openPlugins();

  expect((await plugin("local-auth")).getByText("Enabled")).toBeDefined();
  expect((await plugin("log-events")).getByText("Disabled")).toBeDefined();
  expect((await plugin("tmdb")).getByText("Enabled")).toBeDefined();
});

test("a Plugin's settings fill its form, with defaults as hints and a set secret only as set", async () => {
  openPlugins();

  expect((await input("tmdb", "Language")).value).toBe("de-DE");
  expect((await input("tmdb", "Region")).value).toBe("");
  expect(
    (await plugin("tmdb")).getByText("Where release dates are for. Blank uses US."),
  ).toBeDefined();
  expect((await plugin("tmdb")).getByText("Set")).toBeDefined();
  expect((await plugin("tmdb")).queryByLabelText("API key")).toBeNull();
  expect((await input("tmdb", "Token")).type).toBe("password");
});

test("the form checks the schema: a required setting is required and a number is whole", async () => {
  openPlugins();

  expect((await input("log-events", "Path")).required).toBe(true);
  expect((await input("tmdb", "Language")).required).toBe(false);
  const days = await input("log-events", "Keep for days");
  expect([days.type, days.step]).toEqual(["number", "1"]);
  expect((await input("log-events", "Verbose")).checked).toBe(true);
});

test("saving sends what was typed, leaving out blanks and a set secret", async () => {
  const { sent, submit } = openToSave("tmdb");
  fireEvent.change(await input("tmdb", "Language"), { target: { value: "fr-FR" } });

  await submit();

  const { pluginId, settings } = await sent;
  expect(pluginId).toBe("tmdb");
  expect(Object.keys(settings)).toEqual(["language"]);
  expect(settings["language"]?.value).toEqual({ case: "textValue", value: "fr-FR" });
  expect(await screen.findByText("Saved, and in use now.")).toBeDefined();
});

test("Replace lets a set secret be typed again, and sends it", async () => {
  const { sent, submit } = openToSave("tmdb");
  fireEvent.click((await plugin("tmdb")).getByRole("button", { name: "Replace API key" }));
  fireEvent.change(await input("tmdb", "API key"), { target: { value: "new-key" } });

  await submit();

  expect((await sent).settings["api_key"]?.value).toEqual({ case: "textValue", value: "new-key" });
});

test("a bool and a number, however it is written, are sent as their kind", async () => {
  const { sent, submit } = openToSave("log-events");
  fireEvent.click(await input("log-events", "Verbose"));
  fireEvent.change(await input("log-events", "Keep for days"), { target: { value: "7.0" } });
  fireEvent.change(await input("log-events", "Path"), { target: { value: "/var/log" } });

  await submit();

  const { settings } = await sent;
  expect(settings["verbose"]?.value).toEqual({ case: "boolValue", value: false });
  expect(settings["keep_days"]?.value).toEqual({ case: "integerValue", value: 7n });
});

test("a refusal shows under the setting it names", async () => {
  openPlugins(() => {
    throw refused(Code.InvalidArgument, "the setting path is required", {
      reason: RefusalReason.FIELD_REQUIRED,
      field: "settings.path",
    });
  });
  fireEvent.change(await input("log-events", "Path"), { target: { value: "x" } });

  fireEvent.click((await plugin("log-events")).getByRole("button", { name: "Save" }));

  await expectUnderField("Path", "This setting is required.");
});

test("a refusal naming a secret that is set opens its input to show under", async () => {
  openPlugins(() => {
    throw refused(Code.InvalidArgument, "the setting api_key must be Secret", {
      reason: RefusalReason.WRONG_SETTING_KIND,
      field: "settings.api_key",
    });
  });

  fireEvent.click((await plugin("tmdb")).getByRole("button", { name: "Save" }));

  await expectUnderField("API key", "This setting can’t take that value.");
});
