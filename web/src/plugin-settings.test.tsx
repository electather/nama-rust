import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { LibraryService } from "@/gen/nama/v1/library_pb";
import { SettingKind, type UpdatePluginSettingsRequest } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { input, openPlugin, plugin } from "./test-plugins";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

/**
 * Opens the Plugin `id`'s details page to save its settings, answering `LibraryService` calls with
 * `library` where it gives them: `submit` saves them, and `sent` is the request the Server got.
 */
function openToSave(id: string, library: Partial<ServiceImpl<typeof LibraryService>> = {}) {
  const sent = new Promise<UpdatePluginSettingsRequest>((resolve) => {
    openPlugin(
      id,
      {
        updatePluginSettings: (request) => {
          resolve(request);
          return { plugin: { summary: { id, enabled: true } } };
        },
      },
      library,
    );
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

test("the form checks the schema: a disabled Plugin's required setting may be blank and a number is whole", async () => {
  openPlugin("log-events");

  expect((await input("log-events", "Path")).required).toBe(false);
  const days = await input("log-events", "Keep for days");
  expect([days.type, days.step]).toEqual(["number", "1"]);
  expect((await input("log-events", "Verbose")).checked).toBe(true);
});

test("the form checks the schema: an enabled Plugin's required setting is required", async () => {
  const path = {
    key: "path",
    label: "Path",
    description: "",
    kind: SettingKind.TEXT,
    required: true,
  };
  openPlugin("log-events", {
    getPlugin: () => ({
      plugin: { summary: { id: "log-events", enabled: true }, settings: [path] },
    }),
  });

  expect((await input("log-events", "Path")).required).toBe(true);
});

test("the form checks the schema: a number takes only its bounds", async () => {
  openPlugin("cleanup");

  const days = await input("cleanup", "Days");
  expect([days.min, days.max]).toEqual(["0", ""]);
});

test("a number out of its bounds is refused under it, saying the bound", async () => {
  openPlugin("cleanup", {
    updatePluginSettings: () => {
      throw refused(Code.InvalidArgument, "the setting days must be at least 0", {
        reason: RefusalReason.SETTING_OUT_OF_RANGE,
        field: "settings.days",
        params: { settingKey: "days", min: "0" },
      });
    },
  });

  fireEvent.click((await plugin("cleanup")).getByRole("button", { name: "Save" }));

  await expectUnderField("Days", "This must be at least 0.");
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

test("Forget lets a set secret be left blank to forget it, and sends its key", async () => {
  const { sent, submit } = openToSave("tmdb");
  fireEvent.click((await plugin("TMDB")).getByRole("button", { name: "Forget API key" }));

  expect((await input("TMDB", "API key")).value).toBe("");
  expect(
    (await plugin("TMDB")).getByText("Leave it blank to forget the one that’s set.", {
      exact: false,
    }),
  ).toBeDefined();
  await submit();

  const { settings, forgottenSecrets } = await sent;
  expect(forgottenSecrets).toEqual(["api_key"]);
  expect(settings["api_key"]).toBeUndefined();
});

test("a value typed after Forget is sent in the forgotten secret's place", async () => {
  const { sent, submit } = openToSave("tmdb");
  fireEvent.click((await plugin("TMDB")).getByRole("button", { name: "Forget API key" }));
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

/** The Libraries setting of the cleanup Plugin's details page, once its Libraries show. */
async function librariesToClean() {
  const group = (await plugin("cleanup")).getByRole("group", { name: "Libraries to clean" });
  await within(group).findByRole("checkbox", { name: "Movies" });
  return within(group);
}

test("a Libraries setting shows the Libraries it holds by name, leaving out one since deleted", async () => {
  openPlugin("cleanup");

  const group = await librariesToClean();
  const ticked = group
    .getAllByRole<HTMLInputElement>("checkbox")
    .map((box) => [box.labels?.[0]?.textContent, box.checked]);
  expect(ticked).toEqual([
    ["Movies", true],
    ["Shows", false],
    ["Anime", false],
  ]);
});

test("picking two Libraries saves their ids", async () => {
  const { sent, submit } = openToSave("cleanup");
  const group = await librariesToClean();
  fireEvent.click(group.getByRole("checkbox", { name: "Movies" }));
  fireEvent.click(group.getByRole("checkbox", { name: "Shows" }));
  fireEvent.click(group.getByRole("checkbox", { name: "Anime" }));

  await submit();

  const { settings } = await sent;
  expect(settings["libraries"]?.value.case).toBe("libraryIds");
  expect(settings["libraries"]?.value.value).toMatchObject({ ids: [2n, 3n] });
});

test.each([
  {
    listing: "not yet listed",
    shown: "Loading the Libraries…",
    listLibraries: () => new Promise<never>(() => {}),
  },
  {
    listing: "not listable",
    shown: "The Libraries couldn’t be listed, so saving keeps the ones picked.",
    listLibraries: () => {
      throw new ConnectError("the database is locked", Code.Internal);
    },
  },
])(
  "saving while the Libraries are $listing keeps the ids the setting holds",
  async ({ shown, listLibraries }) => {
    const { sent, submit } = openToSave("cleanup", { listLibraries });
    const group = within(
      (await plugin("cleanup")).getByRole("group", { name: "Libraries to clean" }),
    );
    await group.findByText(shown);

    await submit();

    const { settings } = await sent;
    expect(settings["libraries"]?.value.case).toBe("libraryIds");
    expect(settings["libraries"]?.value.value).toMatchObject({ ids: [1n, 9n] });
  },
);

test("a refusal naming a Library no longer on the Server shows under the Libraries setting", async () => {
  openPlugin("cleanup", {
    updatePluginSettings: () => {
      throw refused(Code.NotFound, "the setting libraries names no Library 9", {
        reason: RefusalReason.LIBRARY_NOT_FOUND,
        field: "settings.libraries",
        params: { libraryId: "9" },
      });
    },
  });
  const group = await librariesToClean();

  fireEvent.click((await plugin("cleanup")).getByRole("button", { name: "Save" }));

  const error = await screen.findByText("A Library picked here isn’t on the Server any more.");
  const box = group.getByRole("checkbox", { name: "Movies" });
  expect(box.getAttribute("aria-invalid")).toBe("true");
  expect(box.getAttribute("aria-describedby")).toContain(error.id);
});
