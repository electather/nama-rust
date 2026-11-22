import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { SettingKind, type UpdatePluginSettingsRequest } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  chipsField,
  offeredIn,
  openPlugin,
  pickedIn,
  plugin,
  saveSettings,
  toggleIn,
} from "@/test-plugins";
import { refused } from "@/test-refusal";

afterEach(cleanup);

/** Presses the test Plugin's Test button. */
async function pressTest() {
  const found = await plugin("test-choices");
  fireEvent.click(found.getByRole("button", { name: "Test the settings" }));
}

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

/** The test Plugin's Ratings field. */
async function ratings() {
  const field = await chipsField("test-choices", "Ratings");
  return field;
}

/** The labels of the Ratings' choices, in order, and of those picked. */
async function shown() {
  const field = await ratings();
  return { offered: await offeredIn(field), picked: pickedIn(field) };
}

/** Picks or removes the Ratings labelled `labels`, in order. */
async function pick(...labels: string[]) {
  for (const label of labels) {
    // oxlint-disable-next-line no-await-in-loop -- each pick saves before the next, as a person's would
    await toggleIn(await ratings(), label);
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

test("before a test, a several-choices setting offers its Manifest’s choices, held ones picked, unmarked", async () => {
  openPlugin("test-choices");

  await expect(shown()).resolves.toStrictEqual({
    offered: ["G", "PG", "R", "nc-17"],
    picked: ["PG", "nc-17"],
  });
});

/** A several-choices value holding `values`. */
const texts = (...values: string[]) => ({ value: { case: "texts" as const, value: { values } } });

/**
 * Opens the test Plugin without a settings check, with only its Ratings, which hold `held` or
 * nothing, keeping the Ratings' values each save sends in `sent`.
 */
function openRatings(
  held: { value?: ReturnType<typeof texts>; defaultValue?: ReturnType<typeof texts> },
  sent: unknown[] = [],
) {
  const summary = { id: "test-choices", enabled: true };
  openPlugin("test-choices", {
    getPlugin: () => ({ plugin: { summary, settings: [{ ...ratingsSetting, ...held }] } }),
    updatePluginSettings: (request) => {
      sent.push(savedRatings(request));
      return { plugin: { summary } };
    },
  });
}

test("without a settings check, a held choice the Manifest doesn’t list is no longer offered", async () => {
  openRatings({ value: texts("pg", "nc-17") });

  await expect(shown()).resolves.toStrictEqual({
    offered: ["G", "PG", "R", "nc-17 (no longer offered)"],
    picked: ["PG", "nc-17 (no longer offered)"],
  });
});

/**
 * A fake Server whose test Plugin holds the Ratings it was last sent, none at first, keeping the
 * Ratings' values each save sends in `sent`.
 */
function ratingsServer(sent: unknown[]): Parameters<typeof openPlugin>[1] {
  let held: string[] = [];
  const summary = { id: "test-choices", enabled: true };
  return {
    getPlugin: () => ({
      plugin: {
        summary,
        settings: [{ ...ratingsSetting, value: texts(...held) }],
      },
    }),
    updatePluginSettings: (request) => {
      held = savedRatings(request) ?? [];
      sent.push(held);
      return { plugin: { summary } };
    },
  };
}

test("each choice picked is saved with the rest, and they show picked after a reload", async () => {
  const sent: unknown[] = [];
  const server = ratingsServer(sent);
  openPlugin("test-choices", server);
  await pick("G", "R");
  // One save a pick, in order, so the last one holds them all.
  await waitFor(() => {
    expect(sent).toStrictEqual([["g"], ["g", "r"]]);
  });
  cleanup();

  openPlugin("test-choices", server);

  await expect(shown()).resolves.toStrictEqual({ offered: ["G", "PG", "R"], picked: ["G", "R"] });
});

test("a several-choices setting holding nothing shows its default picked", async () => {
  openRatings({ defaultValue: texts("g", "r") });

  const { picked } = await shown();
  expect(picked).toStrictEqual(["G", "R"]);
});

test("a several-choices setting holding nothing saves nothing while its default stays picked", async () => {
  const sent: unknown[] = [];
  openRatings({ defaultValue: texts("r", "g") }, sent);

  await shown();
  await saveSettings("test-choices");

  await waitFor(() => {
    expect(sent).toStrictEqual([undefined]);
  });
});

test("a several-choices setting holding nothing saves its picks once they differ from its default", async () => {
  const sent: unknown[] = [];
  openRatings({ defaultValue: texts("g", "r") }, sent);

  await pick("PG");

  await waitFor(() => {
    expect(sent).toStrictEqual([["g", "pg", "r"]]);
  });
});

test("a required several-choices setting left with none picked shows as missing", async () => {
  openPlugin("test-choices", {
    updatePluginSettings: () => {
      throw refused(Code.InvalidArgument, "the setting ratings is required", {
        reason: RefusalReason.FIELD_REQUIRED,
        field: "settings.ratings",
        params: { pluginId: "test-choices", settingKey: "ratings" },
      });
    },
  });
  await pick("PG", "nc-17");

  const found = await plugin("test-choices");
  const error = await found.findByText("This setting is required.");
  const field = await ratings();
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});

test("unpicking every choice saves none picked, not the default", async () => {
  const sent: unknown[] = [];
  openSaving(sent);

  await pick("PG", "nc-17");

  await waitFor(() => {
    expect(sent).toStrictEqual([["nc-17"], []]);
  });
});

test("a held choice no longer offered is saved until it’s unpicked", async () => {
  const sent: unknown[] = [];
  openSaving(sent);

  await pick("G");

  await waitFor(() => {
    expect(sent).toStrictEqual([["g", "pg", "nc-17"]]);
  });
});

test("a passing test’s choices replace the Manifest’s, and a held one it drops is no longer offered", async () => {
  openPlugin("test-choices", {
    testPluginSettings: () => ({
      choices: { ratings: { choices: [{ value: "tv-14", label: "TV-14" }] } },
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
