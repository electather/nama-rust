import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  SettingKind,
  type TestPluginSettingsRequest,
  type UpdatePluginSettingsRequest,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { input, openPlugin, plugin } from "@/test-plugins";
import { expectUnderField, refused } from "@/test-refusal";

afterEach(cleanup);

/** What the test Plugin's check offers for its Profile. */
const profiles = {
  choices: {
    profile: {
      choices: [
        { value: "1", label: "HD" },
        { value: "2", label: "Ultra HD" },
      ],
    },
  },
};

/** A several-choices setting holding nothing, on a Plugin without a settings check. */
const ratingsSetting = {
  key: "ratings",
  label: "Ratings",
  description: "",
  kind: SettingKind.CHOICES,
  choices: [
    { value: "g", label: "G" },
    { value: "pg", label: "PG" },
    { value: "r", label: "R" },
  ],
};

/** The dropdown for the test Plugin's Profile. */
async function profile() {
  const found101 = await plugin("test-choices");
  return found101.getByLabelText<HTMLSelectElement>("Profile");
}

async function pressSave() {
  const found1 = await plugin("test-choices");
  fireEvent.click(found1.getByRole("button", { name: "Save" }));
}

async function pressTest() {
  const found2 = await plugin("test-choices");
  fireEvent.click(found2.getByRole("button", { name: "Test" }));
}

test("a Plugin with a settings check has a Test button", async () => {
  openPlugin("test-choices");

  const found3 = await plugin("test-choices");
  expect(found3.getByRole("button", { name: "Test" })).toBeDefined();
});

test("a Plugin without a settings check has no Test button", async () => {
  openPlugin("tmdb");

  const found4 = await plugin("TMDB");
  expect(found4.queryByRole("button", { name: "Test" })).toBeNull();
});

test("a choice can’t be picked before a test, and a passing test fills its dropdown", async () => {
  const tested = new Promise<TestPluginSettingsRequest>((resolve) => {
    openPlugin("test-choices", {
      testPluginSettings: (request) => {
        resolve(request);
        return profiles;
      },
    });
  });
  const found5 = await profile();
  expect(found5.disabled).toBe(true);
  const found6 = await plugin("test-choices");
  fireEvent.click(found6.getByRole("button", { name: "Replace API key" }));
  fireEvent.change(await input("test-choices", "API key"), { target: { value: "unsaved" } });

  await pressTest();

  await waitFor(async () => {
    const found7 = await profile();
    expect(found7.disabled).toBe(false);
  });
  const found102 = await profile();
  const options = [...found102.options].map((option) => option.text);
  expect(options).toStrictEqual(["Pick one", "HD", "Ultra HD"]);
  const found8 = await profile();
  expect(found8.value).toBe("1");
  const found9 = await tested;
  expect(found9.settings["api_key"]?.value).toStrictEqual({
    case: "textValue",
    value: "unsaved",
  });
});

test("a picked choice is saved, and an untested one is saved as it was", async () => {
  const sent: UpdatePluginSettingsRequest[] = [];
  openPlugin("test-choices", {
    testPluginSettings: () => profiles,
    updatePluginSettings: (request) => {
      sent.push(request);
      return { plugin: { summary: { id: "test-choices", enabled: true } } };
    },
  });
  await pressSave();
  await pressTest();
  await waitFor(async () => {
    const found10 = await profile();
    expect(found10.disabled).toBe(false);
  });
  fireEvent.change(await profile(), { target: { value: "2" } });
  await pressSave();

  await waitFor(() => {
    expect(sent.map(({ settings }) => settings["profile"]?.value.value)).toStrictEqual(["1", "2"]);
  });
});

test("a failed test shows the Plugin’s error and leaves the choice locked", async () => {
  openPlugin("test-choices", {
    testPluginSettings: () => {
      // A Plugin's message may end its own sentence.
      throw refused(Code.FailedPrecondition, "the service refused the API key.", {
        reason: RefusalReason.SETTINGS_CHECK_FAILED,
        params: { pluginId: "test-choices" },
      });
    },
  });

  await pressTest();

  const found11 = await plugin("test-choices");
  await expect(found11.findByText("The service refused the API key.")).resolves.toBeDefined();
  const found12 = await profile();
  expect(found12.disabled).toBe(true);
});

test("a passing test that offers nothing for a choice unlocks it with nothing to pick", async () => {
  openPlugin("test-choices", { testPluginSettings: () => ({ choices: {} }) });

  await pressTest();

  await waitFor(async () => {
    const found13 = await profile();
    expect(found13.disabled).toBe(false);
  });
  const found14 = await profile();
  expect([...found14.options].map((option) => option.text)).toStrictEqual(["Pick one"]);
});

/** Saves the test Plugin's settings, refused for `reason` with `message`, naming its Profile. */
async function saveRefusedOnProfile(reason: RefusalReason, message: string) {
  openPlugin("test-choices", {
    updatePluginSettings: () => {
      throw refused(Code.FailedPrecondition, message, {
        reason,
        field: "settings.profile",
        params: { pluginId: "test-choices", settingKey: "profile" },
      });
    },
  });
  await pressSave();
}

test("a save refused for a failed check shows the Plugin’s error on the choice it names", async () => {
  await saveRefusedOnProfile(
    RefusalReason.SETTINGS_CHECK_FAILED,
    "the service refused the API key",
  );

  await expectUnderField("Profile", "The service refused the API key.");
});

test("a save refused for a choice the Plugin doesn’t offer says so on that choice", async () => {
  await saveRefusedOnProfile(
    RefusalReason.CHOICE_NOT_OFFERED,
    "1 isn't one of the choices for the setting profile",
  );

  await expectUnderField(
    "Profile",
    "The Plugin doesn’t offer this choice now. Test the settings and pick again.",
  );
});

/** The test Plugin's Ratings. */
async function ratings() {
  const found = await plugin("test-choices");
  return within(found.getByRole("group", { name: "Ratings" }));
}

/** The labels of the Ratings' choices, in order, and of those picked. */
async function shown() {
  const group = await ratings();
  const boxes = group.getAllByRole<HTMLInputElement>("checkbox");
  return {
    offered: boxes.map((box) => box.labels?.[0]?.textContent),
    picked: boxes.filter(({ checked }) => checked).map((box) => box.labels?.[0]?.textContent),
  };
}

/** Ticks or unticks the Ratings labelled `labels`. */
async function pick(...labels: string[]) {
  const group = await ratings();
  for (const name of labels) {
    fireEvent.click(group.getByRole("checkbox", { name }));
  }
}

/** The Ratings' values `request` saves, if it holds a several-choices value for them. */
function savedRatings({ settings }: UpdatePluginSettingsRequest) {
  const saved = settings["ratings"]?.value;
  return saved?.case === "texts" ? saved.value.values : undefined;
}

/** Opens the test Plugin, keeping the Ratings' values each save sends in `sent`. */
function openSaving(sent: unknown[]) {
  openPlugin("test-choices", {
    updatePluginSettings: (request) => {
      sent.push(savedRatings(request));
      return { plugin: { summary: { id: "test-choices", enabled: true } } };
    },
  });
}

test("a several-choices setting offers its Manifest’s choices before a test, held ones picked", async () => {
  openPlugin("test-choices");

  await expect(shown()).resolves.toStrictEqual({
    offered: ["G", "PG", "R", "nc-17 (no longer offered)"],
    picked: ["PG", "nc-17 (no longer offered)"],
  });
});

/** A fake Server whose test Plugin holds the Ratings it was last sent, none at first. */
function ratingsServer(): Parameters<typeof openPlugin>[1] {
  let held: string[] = [];
  const summary = { id: "test-choices", enabled: true };
  return {
    getPlugin: () => ({
      plugin: {
        summary,
        settings: [
          { ...ratingsSetting, value: { value: { case: "texts", value: { values: held } } } },
        ],
      },
    }),
    updatePluginSettings: (request) => {
      held = savedRatings(request) ?? [];
      return { plugin: { summary } };
    },
  };
}

test("several picked choices are saved, and show picked after a reload", async () => {
  const server = ratingsServer();
  openPlugin("test-choices", server);
  await pick("G", "R");
  await pressSave();
  const found = await plugin("test-choices");
  await found.findByText("Saved, and in use now.");
  cleanup();

  openPlugin("test-choices", server);

  await expect(shown()).resolves.toStrictEqual({ offered: ["G", "PG", "R"], picked: ["G", "R"] });
});

test("a several-choices setting holding nothing shows its default picked", async () => {
  const defaultValue = { value: { case: "texts" as const, value: { values: ["g", "r"] } } };
  openPlugin("test-choices", {
    getPlugin: () => ({
      plugin: {
        summary: { id: "test-choices", enabled: true },
        settings: [{ ...ratingsSetting, defaultValue }],
      },
    }),
  });

  const { picked } = await shown();
  expect(picked).toStrictEqual(["G", "R"]);
});

test("a required several-choices setting saved with none picked shows as missing", async () => {
  openPlugin("test-choices", {
    updatePluginSettings: () => {
      throw refused(Code.InvalidArgument, "the setting ratings is required", {
        reason: RefusalReason.FIELD_REQUIRED,
        field: "settings.ratings",
        params: { pluginId: "test-choices", settingKey: "ratings" },
      });
    },
  });
  await pick("PG", "nc-17 (no longer offered)");

  await pressSave();

  const group = await ratings();
  const error = await group.findByText("This setting is required.");
  const found = await plugin("test-choices");
  const fieldset = found.getByRole("group", { name: "Ratings" });
  expect(fieldset.getAttribute("aria-describedby")).toContain(error.id);
});

test("unpicking every choice saves none picked, not the default", async () => {
  const sent: unknown[] = [];
  openSaving(sent);

  await pick("PG", "nc-17 (no longer offered)");
  await pressSave();

  await waitFor(() => {
    expect(sent).toStrictEqual([[]]);
  });
});

test("a held choice no longer offered is saved until it’s unpicked", async () => {
  const sent: unknown[] = [];
  openSaving(sent);

  await pick("G");
  await pressSave();

  await waitFor(() => {
    expect(sent).toStrictEqual([["g", "pg", "nc-17"]]);
  });
});

test("a passing test’s choices replace the Manifest’s, and a held one it drops is no longer offered", async () => {
  openPlugin("test-choices", {
    testPluginSettings: () => ({
      choices: { ...profiles.choices, ratings: { choices: [{ value: "tv-14", label: "TV-14" }] } },
    }),
  });

  await pressTest();

  await waitFor(async () => {
    await expect(shown()).resolves.toStrictEqual({
      offered: ["TV-14", "pg (no longer offered)", "nc-17 (no longer offered)"],
      picked: ["pg (no longer offered)", "nc-17 (no longer offered)"],
    });
  });
});
