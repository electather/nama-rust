import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { UpdatePluginSettingsRequest } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { input, openPlugin, plugin } from "./test-plugins";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

/**
 * Opens the Plugin `id`'s details page to save its settings: `submit` saves them, and `sent` is the
 * request the Server got.
 */
function openToSave(id: string) {
  const sent = new Promise<UpdatePluginSettingsRequest>((resolve) => {
    openPlugin(id, {
      updatePluginSettings: (request) => {
        resolve(request);
        return { plugin: { id, enabled: true } };
      },
    });
  });
  return {
    sent,
    submit: async () => fireEvent.click(await screen.findByRole("button", { name: "Save" })),
  };
}

test("a Plugin's settings fill its form, with defaults as hints and a set secret only as set", async () => {
  openPlugin("tmdb");

  expect((await input("TMDB", "Language")).value).toBe("de-DE");
  expect((await input("TMDB", "Region")).value).toBe("");
  expect(
    (await plugin("TMDB")).getByText("Where release dates are for. Blank uses US."),
  ).toBeDefined();
  expect((await plugin("TMDB")).getByText("Set")).toBeDefined();
  expect((await plugin("TMDB")).queryByLabelText("API key")).toBeNull();
  expect((await input("TMDB", "Token")).type).toBe("password");
});

test("the form checks the schema: a required setting is required and a number is whole", async () => {
  openPlugin("log-events");

  expect((await input("log-events", "Path")).required).toBe(true);
  const days = await input("log-events", "Keep for days");
  expect([days.type, days.step]).toEqual(["number", "1"]);
  expect((await input("log-events", "Verbose")).checked).toBe(true);
});

test("the form checks the schema: a setting with a default isn't required and a URL is a URL", async () => {
  openPlugin("test-plugin");

  const url = await input("test-plugin", "Sonarr URL");
  expect([url.type, url.value, url.required]).toEqual(["url", "http://sonarr.lan:8989", false]);
});

test("saving sends what was typed, leaving out blanks and a set secret", async () => {
  const { sent, submit } = openToSave("tmdb");
  fireEvent.change(await input("TMDB", "Language"), { target: { value: "fr-FR" } });

  await submit();

  const { id, settings } = await sent;
  expect(id).toBe("tmdb");
  expect(Object.keys(settings)).toEqual(["language"]);
  expect(settings["language"]?.value).toEqual({ case: "textValue", value: "fr-FR" });
  expect(await screen.findByText("Saved, and in use now.")).toBeDefined();
});

test("Replace lets a set secret be typed again, and sends it", async () => {
  const { sent, submit } = openToSave("tmdb");
  fireEvent.click((await plugin("TMDB")).getByRole("button", { name: "Replace API key" }));
  fireEvent.change(await input("TMDB", "API key"), { target: { value: "new-key" } });

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
  openPlugin("log-events", {
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
  openPlugin("tmdb", {
    updatePluginSettings: () => {
      throw refused(Code.InvalidArgument, "the setting api_key must be Secret", {
        reason: RefusalReason.SETTING_WRONG_KIND,
        field: "settings.api_key",
      });
    },
  });

  fireEvent.click((await plugin("TMDB")).getByRole("button", { name: "Save" }));

  await expectUnderField("API key", "This setting can’t take that value.");
});

test("a Plugin no longer on the Server says so for the form", async () => {
  openPlugin("tmdb", {
    updatePluginSettings: () => {
      throw refused(Code.NotFound, "there is no Plugin tmdb", {
        reason: RefusalReason.PLUGIN_NOT_FOUND,
        field: "id",
      });
    },
  });

  fireEvent.click((await plugin("TMDB")).getByRole("button", { name: "Save" }));

  expect(await screen.findByText("This Plugin isn’t on the Server any more.")).toBeDefined();
});
