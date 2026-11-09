import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  ExtensionPoint,
  PluginKind,
  type ReplaceQualitiesRequest,
  SettingKind,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { openPlugin, plugin } from "./test-plugins";
import { refused } from "./test-refusal";

afterEach(cleanup);

/** Radarr, set up with its Quality profile setting. */
const radarr = {
  id: "radarr",
  name: "Radarr",
  version: "0.1.0",
  kind: PluginKind.INSTALLED,
  enabled: true,
  extensionPoints: [ExtensionPoint.MOVIE_FULFILLER],
  checksSettings: true,
  settings: [
    {
      key: "quality_profile",
      label: "Quality profile",
      description: "",
      kind: SettingKind.CHOICE,
      required: true,
      value: { value: { case: "textValue" as const, value: "1" } },
    },
  ],
};

/**
 * Radarr's page, offering HD (the default) and the profiles `profiles`, HD-1080p and Ultra-HD
 * unless given, answering a save with `replace`.
 */
function openRadarr(
  replace: (request: ReplaceQualitiesRequest) => object,
  profiles = [
    { value: "4", label: "HD-1080p" },
    { value: "5", label: "Ultra-HD" },
  ],
) {
  openPlugin("radarr", {
    getPlugin: () => ({ plugin: radarr }),
    listQualities: () => ({
      qualities: [{ id: 3n, profileId: "4", name: "HD", description: "", isDefault: true }],
    }),
    testPluginSettings: () => ({
      choices: {
        quality_profile: { choices: profiles },
      },
    }),
    replaceQualities: replace,
  });
}

/** The fields of the `number`th Quality on Radarr's page. */
async function quality(number: number) {
  const page = await plugin("Radarr");
  return within(await page.findByRole("group", { name: `Quality ${number}` }));
}

test("an Admin adds a Quality from the service's profiles and saves the list with its default", async () => {
  const sent = new Promise<ReplaceQualitiesRequest>((resolve) => {
    openRadarr((request) => {
      resolve(request);
      return { qualities: request.qualities };
    });
  });
  const page = await plugin("Radarr");
  await waitFor(async () => {
    expect((await quality(1)).getByRole("option", { name: "Ultra-HD" })).toBeDefined();
  });

  fireEvent.click(page.getByRole("button", { name: "Add a Quality" }));
  const added = await quality(2);
  fireEvent.change(added.getByLabelText("Profile"), { target: { value: "5" } });
  fireEvent.change(added.getByLabelText("Name"), { target: { value: "4K" } });
  fireEvent.click(page.getByRole("button", { name: "Save Qualities" }));

  const { id, qualities } = await sent;
  expect(id).toBe("radarr");
  expect(
    qualities.map(({ id: qualityId, profileId, name, isDefault }) => [
      qualityId,
      profileId,
      name,
      isDefault,
    ]),
  ).toEqual([
    [3n, "4", "HD", true],
    [0n, "5", "4K", false],
  ]);
});

test("a Quality whose profile the service no longer offers is flagged", async () => {
  openRadarr(() => ({}), [{ value: "5", label: "Ultra-HD" }]);
  await plugin("Radarr");

  const hd = await quality(1);

  const flag = await hd.findByText("Radarr doesn’t offer this profile now.");
  expect(hd.getByLabelText("Profile").getAttribute("aria-describedby")).toContain(flag.id);
});

test("a name another Quality has is refused under it", async () => {
  openRadarr(() => {
    throw refused(Code.InvalidArgument, "two Qualities have the same name", {
      reason: RefusalReason.QUALITY_REPEATED,
      field: "qualities[0].name",
    });
  });
  const page = await plugin("Radarr");
  await quality(1);

  fireEvent.click(page.getByRole("button", { name: "Save Qualities" }));

  const error = await page.findByText("Another Quality has this name.");
  expect((await quality(1)).getByLabelText("Name").getAttribute("aria-describedby")).toContain(
    error.id,
  );
});

test("a Plugin without a Quality profile setting has no Qualities", async () => {
  openPlugin("test-choices");

  await plugin("test-choices");

  expect(screen.queryByRole("heading", { name: "Qualities" })).toBeNull();
});
