import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type {
  TestPluginSettingsRequest,
  UpdatePluginSettingsRequest,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { input, openPlugins, plugin } from "./test-plugins";
import { refused } from "./test-refusal";

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

/** The dropdown for the test Plugin's Profile. */
async function profile() {
  return (await plugin("test-choices")).getByLabelText<HTMLSelectElement>("Profile");
}

async function pressSave() {
  fireEvent.click((await plugin("test-choices")).getByRole("button", { name: "Save" }));
}

async function pressTest() {
  fireEvent.click((await plugin("test-choices")).getByRole("button", { name: "Test" }));
}

test("only a Plugin with a settings check has a Test button", async () => {
  openPlugins();

  expect((await plugin("test-choices")).getByRole("button", { name: "Test" })).toBeDefined();
  expect((await plugin("tmdb")).queryByRole("button", { name: "Test" })).toBeNull();
});

test("a choice can’t be picked before a test, and a passing test fills its dropdown", async () => {
  const tested = new Promise<TestPluginSettingsRequest>((resolve) => {
    openPlugins({
      testPluginSettings: (request) => {
        resolve(request);
        return profiles;
      },
    });
  });
  expect((await profile()).disabled).toBe(true);
  fireEvent.click((await plugin("test-choices")).getByRole("button", { name: "Replace API key" }));
  fireEvent.change(await input("test-choices", "API key"), { target: { value: "unsaved" } });

  await pressTest();

  await waitFor(async () => {
    expect((await profile()).disabled).toBe(false);
  });
  const options = [...(await profile()).options].map((option) => option.text);
  expect(options).toEqual(["Pick one", "HD", "Ultra HD"]);
  expect((await profile()).value).toBe("1");
  expect((await tested).settings["api_key"]?.value).toEqual({
    case: "textValue",
    value: "unsaved",
  });
});

test("a picked choice is saved, and an untested one is saved as it was", async () => {
  const sent: UpdatePluginSettingsRequest[] = [];
  openPlugins({
    testPluginSettings: () => profiles,
    updatePluginSettings: (request) => {
      sent.push(request);
      return { plugin: { id: "test-choices", enabled: true } };
    },
  });
  await pressSave();
  await pressTest();
  await waitFor(async () => {
    expect((await profile()).disabled).toBe(false);
  });
  fireEvent.change(await profile(), { target: { value: "2" } });
  await pressSave();

  await waitFor(() => {
    expect(sent.map(({ settings }) => settings["profile"]?.value.value)).toEqual(["1", "2"]);
  });
});

test("a failed test shows the Plugin’s error and leaves the choice locked", async () => {
  openPlugins({
    testPluginSettings: () => {
      // A Plugin's message may end its own sentence.
      throw refused(Code.FailedPrecondition, "the service refused the API key.", {
        reason: RefusalReason.SETTINGS_CHECK_FAILED,
        params: { pluginId: "test-choices" },
      });
    },
  });

  await pressTest();

  expect(
    await (await plugin("test-choices")).findByText("The service refused the API key."),
  ).toBeDefined();
  expect((await profile()).disabled).toBe(true);
});

test("a passing test that offers nothing for a choice unlocks it with nothing to pick", async () => {
  openPlugins({ testPluginSettings: () => ({ choices: {} }) });

  await pressTest();

  await waitFor(async () => {
    expect((await profile()).disabled).toBe(false);
  });
  expect([...(await profile()).options].map((option) => option.text)).toEqual(["Pick one"]);
});
