import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { LibraryService } from "@/gen/nama/v1/library_pb";
import { SettingKind, type UpdatePluginSettingsRequest } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { input, openPlugin, plugin, saveSettings } from "@/test-plugins";
import { expectUnderField, refused } from "@/test-refusal";

afterEach(cleanup);

/**
 * Opens the Plugin `id`'s details page to change its settings, answering `LibraryService` calls
 * with `library` where it gives them: `sent` is the first request to save them the Server got, and
 * `requests` every one, in order.
 */
function openToSave(id: string, library: Partial<ServiceImpl<typeof LibraryService>> = {}) {
  const requests: UpdatePluginSettingsRequest[] = [];
  const sent = new Promise<UpdatePluginSettingsRequest>((resolve) => {
    openPlugin(
      id,
      {
        updatePluginSettings: (request) => {
          requests.push(request);
          resolve(request);
          return { plugin: { summary: { id, enabled: true } } };
        },
      },
      library,
    );
  });
  return { sent, requests };
}

/** Types `value` into the setting `label` of the Plugin shown as `name`, then leaves the field. */
async function typeAndLeave(name: string, label: string, value: string) {
  const field = await input(name, label);
  fireEvent.change(field, { target: { value } });
  fireEvent.blur(field);
}

test("a Plugin's settings fill its form, with defaults as hints and a set secret only as set", async () => {
  openPlugin("tmdb");

  const found1 = await input("TMDB", "Language");
  expect(found1.value).toBe("de-DE");
  const found2 = await input("TMDB", "Region");
  expect(found2.value).toBe("");
  const found3 = await plugin("TMDB");
  expect(found3.getByText("Where release dates are for. Blank uses US.")).toBeDefined();
  const found4 = await plugin("TMDB");
  expect(found4.getByText("Set")).toBeDefined();
  const found5 = await plugin("TMDB");
  expect(found5.queryByLabelText("API key")).toBeNull();
  const found6 = await input("TMDB", "Token");
  expect(found6.type).toBe("password");
});

test("the form checks the schema: a disabled Plugin's required setting may be blank and a number is whole", async () => {
  openPlugin("log-events");

  const found7 = await input("log-events", "Path");
  expect(found7.required).toBe(false);
  const days = await input("log-events", "Keep for days");
  expect([days.type, days.step]).toStrictEqual(["number", "1"]);
  const page = await plugin("log-events");
  const verbose = page.getByRole("switch", { name: "Verbose" });
  expect(verbose.getAttribute("aria-checked")).toBe("true");
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

  const found9 = await input("log-events", "Path");
  expect(found9.required).toBe(true);
});

test("the form checks the schema: a number takes only its bounds", async () => {
  openPlugin("cleanup");

  const days = await input("cleanup", "Days");
  expect([days.min, days.max]).toStrictEqual(["0", ""]);
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

  await typeAndLeave("cleanup", "Days", "5");

  await expectUnderField("Days", "This must be at least 0.");
});

test("the form checks the schema: a setting with a default isn't required and a URL is a URL", async () => {
  openPlugin("test-plugin");

  const url = await input("test-plugin", "Sonarr URL");
  expect([url.type, url.value, url.required]).toStrictEqual([
    "url",
    "http://sonarr.lan:8989",
    false,
  ]);
});

test("leaving a field sends what was typed, leaving out blanks and a set secret", async () => {
  const { sent, requests } = openToSave("tmdb");
  const language = await input("TMDB", "Language");
  fireEvent.change(language, { target: { value: "fr-FR" } });
  // Nothing is saved while it's still being typed.
  expect(requests).toStrictEqual([]);

  fireEvent.blur(language);

  const { id, settings } = await sent;
  expect(id).toBe("tmdb");
  expect(Object.keys(settings)).toStrictEqual(["language"]);
  expect(settings["language"]?.value).toStrictEqual({ case: "textValue", value: "fr-FR" });
  const page = await plugin("TMDB");
  expect(page.queryByRole("button", { name: "Save" })).toBeNull();
});

test("enter in a field sends what was typed", async () => {
  const { sent } = openToSave("tmdb");
  const language = await input("TMDB", "Language");
  fireEvent.change(language, { target: { value: "it-IT" } });

  fireEvent.keyDown(language, { key: "Enter" });

  const { settings } = await sent;
  expect(settings["language"]?.value).toStrictEqual({ case: "textValue", value: "it-IT" });
});

test("replace lets a set secret be typed again, and sends it", async () => {
  const { sent } = openToSave("tmdb");
  const found11 = await plugin("TMDB");
  fireEvent.click(found11.getByRole("button", { name: "Replace API key" }));
  await typeAndLeave("TMDB", "API key", "new-key");

  const found12 = await sent;
  expect(found12.settings["api_key"]?.value).toStrictEqual({ case: "textValue", value: "new-key" });
});

test("forget forgets a set secret at the press, sending its key", async () => {
  const { sent } = openToSave("tmdb");
  const found13 = await plugin("TMDB");
  fireEvent.click(found13.getByRole("button", { name: "Forget API key" }));

  const { settings, forgottenSecrets } = await sent;
  expect(forgottenSecrets).toStrictEqual(["api_key"]);
  expect(settings["api_key"]).toBeUndefined();
});

test("a bool and a number, however it is written, are sent as their kind", async () => {
  const { sent } = openToSave("log-events");
  fireEvent.change(await input("log-events", "Keep for days"), { target: { value: "7.0" } });
  fireEvent.change(await input("log-events", "Path"), { target: { value: "/var/log" } });
  const page = await plugin("log-events");
  // A switch saves as it's flipped, with whatever is typed beside it.
  fireEvent.click(page.getByRole("switch", { name: "Verbose" }));

  const { settings } = await sent;
  expect(settings["verbose"]?.value).toStrictEqual({ case: "boolValue", value: false });
  expect(settings["keep_days"]?.value).toStrictEqual({ case: "integerValue", value: 7n });
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
  await typeAndLeave("log-events", "Path", "x");

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

  await saveSettings("TMDB");

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

  await saveSettings("TMDB");

  await expect(
    screen.findByText("This Plugin isn’t on the Server any more."),
  ).resolves.toBeDefined();
});

/** The Libraries setting of the cleanup Plugin's details page, once its Libraries show. */
async function librariesToClean() {
  const found101 = await plugin("cleanup");
  const group = found101.getByRole("group", { name: "Libraries to clean" });
  await within(group).findByRole("checkbox", { name: "Movies" });
  return within(group);
}

test("a Libraries setting shows the Libraries it holds by name, leaving out one since deleted", async () => {
  openPlugin("cleanup");

  const group = await librariesToClean();
  const ticked = group
    .getAllByRole<HTMLInputElement>("checkbox")
    .map((box) => [box.labels?.[0]?.textContent, box.checked]);
  expect(ticked).toStrictEqual([
    ["Movies", true],
    ["Shows", false],
    ["Anime", false],
  ]);
});

test("each Library ticked or unticked saves the ids picked", async () => {
  const { requests } = openToSave("cleanup");
  const group = await librariesToClean();
  fireEvent.click(group.getByRole("checkbox", { name: "Movies" }));
  fireEvent.click(group.getByRole("checkbox", { name: "Shows" }));
  fireEvent.click(group.getByRole("checkbox", { name: "Anime" }));

  await waitFor(() => {
    expect(requests).toHaveLength(3);
  });
  // One save at a time, in the order they were made, so the last one made is the one kept.
  const picked = requests.map(({ settings }) => settings["libraries"]?.value.value);
  expect(picked[0]).toBeUndefined();
  expect(picked[1]).toMatchObject({ ids: [2n] });
  expect(picked[2]).toMatchObject({ ids: [2n, 3n] });
});

test.each([
  {
    listing: "not yet listed",
    shown: "Loading the Libraries…",
    // oxlint-disable-next-line require-await -- never answers, so the Libraries stay loading
    listLibraries: async () => new Promise<never>(() => {}),
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
    const { sent } = openToSave("cleanup", { listLibraries });
    const found21 = await plugin("cleanup");
    const group = within(found21.getByRole("group", { name: "Libraries to clean" }));
    await group.findByText(shown);

    await saveSettings("cleanup");

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

  await saveSettings("cleanup");

  const error = await screen.findByText("A Library picked here isn’t on the Server any more.");
  const box = group.getByRole("checkbox", { name: "Movies" });
  expect(box.getAttribute("aria-invalid")).toBe("true");
  expect(box.getAttribute("aria-describedby")).toContain(error.id);
});
