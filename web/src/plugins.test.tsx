import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { UpdatePluginSettingsRequest } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { input, openPlugins, plugin } from "./test-plugins";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

/**
 * Opens the Plugins page to save the Plugin `id`'s settings: `submit` saves them, and `sent` is
 * the request the Server got.
 */
function openToSave(id: string) {
  const sent = new Promise<UpdatePluginSettingsRequest>((resolve) => {
    openPlugins({
      updatePluginSettings: (request) => {
        resolve(request);
        return { plugin: { id, enabled: true } };
      },
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

test("each Plugin says whether it is Built-in or Installed", async () => {
  openPlugins();

  expect((await plugin("tmdb")).getByText("Built-in, version 0.1.0")).toBeDefined();
  expect((await plugin("test-plugin")).getByText("Installed, version 0.1.0")).toBeDefined();
});

test("a refused Plugin says why and offers no way to enable it", async () => {
  openPlugins();

  const section = await plugin("out-of-range.wasm");
  expect(section.getByText("Refused")).toBeDefined();
  expect(
    section.getByText(
      "The Server can’t run it: it wants nama-api <0.1, and this Server has 0.1.0.",
    ),
  ).toBeDefined();
  expect(section.queryByRole("button")).toBeNull();
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

  const { id, settings } = await sent;
  expect(id).toBe("tmdb");
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
  openPlugins({
    updatePluginSettings: () => {
      throw refused(Code.InvalidArgument, "the setting path is required", {
        reason: RefusalReason.FIELD_REQUIRED,
        field: "settings.path",
      });
    },
  });
  fireEvent.change(await input("log-events", "Path"), { target: { value: "x" } });

  fireEvent.click((await plugin("log-events")).getByRole("button", { name: "Save" }));

  await expectUnderField("Path", "This setting is required.");
});

test("a refusal naming a secret that is set opens its input to show under", async () => {
  openPlugins({
    updatePluginSettings: () => {
      throw refused(Code.InvalidArgument, "the setting api_key must be Secret", {
        reason: RefusalReason.SETTING_WRONG_KIND,
        field: "settings.api_key",
      });
    },
  });

  fireEvent.click((await plugin("tmdb")).getByRole("button", { name: "Save" }));

  await expectUnderField("API key", "This setting can’t take that value.");
});

test("a Plugin no longer on the Server says so for the form", async () => {
  openPlugins({
    updatePluginSettings: () => {
      throw refused(Code.NotFound, "there is no Plugin tmdb", {
        reason: RefusalReason.PLUGIN_NOT_FOUND,
        field: "id",
      });
    },
  });

  fireEvent.click((await plugin("tmdb")).getByRole("button", { name: "Save" }));

  expect(await screen.findByText("This Plugin isn’t on the Server any more.")).toBeDefined();
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

  // Its settings form has an alert under each input too, empty until a save is refused.
  const alert = await logEvents.findByText("It needs Path set before it can be enabled.");
  expect(alert.getAttribute("role")).toBe("alert");
});
