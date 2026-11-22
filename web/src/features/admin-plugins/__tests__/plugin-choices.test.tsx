import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import type {
  TestPluginSettingsRequest,
  UpdatePluginSettingsRequest,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { input, openPlugin, plugin, saveSettings } from "@/test-plugins";
import { expectUnderField, refused } from "@/test-refusal";
import { dropdown, offered, pick } from "@/test-settings";

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
  await plugin("test-choices");
  return dropdown("Profile");
}

async function pressTest() {
  const found2 = await plugin("test-choices");
  fireEvent.click(found2.getByRole("button", { name: "Test the settings" }));
}

test("a Plugin with a settings check has a Test button", async () => {
  openPlugin("test-choices");

  const found3 = await plugin("test-choices");
  expect(found3.getByRole("button", { name: "Test the settings" })).toBeDefined();
});

test("a Plugin without a settings check has no Test button", async () => {
  openPlugin("tmdb");

  const found4 = await plugin("TMDB");
  expect(found4.queryByRole("button", { name: "Test the settings" })).toBeNull();
});

test("a passing check on the stored settings fills a choice's dropdown as the Plugin opens", async () => {
  openPlugin("test-choices", { testPluginSettings: () => profiles });

  await waitFor(async () => {
    const found = await profile();
    expect(found.hasAttribute("disabled")).toBe(false);
  });
  await expect(offered("Profile")).resolves.toStrictEqual(["Pick one", "HD", "Ultra HD"]);
});

test("a choice can’t be picked until a check passes, and a passing test fills its dropdown", async () => {
  const tested = new Promise<TestPluginSettingsRequest>((resolve) => {
    // The check on the stored settings, as the Plugin opens, fails; the test of what's typed passes.
    const check = vi
      .fn<(request: TestPluginSettingsRequest) => typeof profiles>((request) => {
        resolve(request);
        return profiles;
      })
      .mockImplementationOnce(() => {
        throw new ConnectError("no API key", Code.FailedPrecondition);
      });
    openPlugin("test-choices", { testPluginSettings: check });
  });
  const found5 = await profile();
  expect(found5.hasAttribute("disabled")).toBe(true);
  const found6 = await plugin("test-choices");
  fireEvent.click(found6.getByRole("button", { name: "Replace API key" }));
  fireEvent.change(await input("test-choices", "API key"), { target: { value: "unsaved" } });

  await pressTest();

  await waitFor(async () => {
    const found7 = await profile();
    expect(found7.hasAttribute("disabled")).toBe(false);
  });
  await expect(offered("Profile")).resolves.toStrictEqual(["Pick one", "HD", "Ultra HD"]);
  const found8 = await profile();
  expect(found8.textContent).toBe("HD");
  const found9 = await tested;
  expect(found9.settings["api_key"]?.value).toStrictEqual({
    case: "textValue",
    value: "unsaved",
  });
});

test("a choice is saved as it's picked, and an untested one is saved as it was", async () => {
  const sent: UpdatePluginSettingsRequest[] = [];
  openPlugin("test-choices", {
    testPluginSettings: () => profiles,
    updatePluginSettings: (request) => {
      sent.push(request);
      return { plugin: { summary: { id: "test-choices", enabled: true } } };
    },
  });
  await saveSettings("test-choices");
  await pressTest();
  await waitFor(async () => {
    const found10 = await profile();
    expect(found10.hasAttribute("disabled")).toBe(false);
  });
  pick("Profile", "Ultra HD");

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
  expect(found12.hasAttribute("disabled")).toBe(true);
});

test("a passing test that offers nothing for a choice unlocks it with nothing to pick", async () => {
  openPlugin("test-choices", { testPluginSettings: () => ({ choices: {} }) });

  await pressTest();

  await waitFor(async () => {
    const found13 = await profile();
    expect(found13.hasAttribute("disabled")).toBe(false);
  });
  await expect(offered("Profile")).resolves.toStrictEqual(["Pick one"]);
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
  await saveSettings("test-choices");
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
