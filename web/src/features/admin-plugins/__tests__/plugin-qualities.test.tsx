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
import { openPlugin, plugin } from "@/test-plugins";
import { refused } from "@/test-refusal";
import { pick } from "@/test-settings";

afterEach(cleanup);

/** Radarr, set up with its Quality profile setting. */
const radarr = {
  summary: {
    id: "radarr",
    name: "Radarr",
    version: "0.1.0",
    kind: PluginKind.INSTALLED,
    enabled: true,
    extensionPoints: [ExtensionPoint.MOVIE_FULFILLER],
  },
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

const HD = {
  id: 3n,
  profileId: "4",
  name: "HD",
  description: "About 8 GB per film",
  isDefault: true,
};
const UHD = { id: 7n, profileId: "5", name: "4K", description: "", isDefault: false };

const OFFERED = [
  { value: "4", label: "HD-1080p" },
  { value: "5", label: "Ultra-HD" },
];

/**
 * Opens Radarr's page with the Qualities `qualities`, its service offering `profiles`, answering a
 * save with `replace`. Gives every list the Server was sent, in order.
 */
function openRadarr({
  qualities = [HD],
  profiles = OFFERED,
  replace = (request: ReplaceQualitiesRequest): object => ({ qualities: request.qualities }),
} = {}) {
  const sent: ReplaceQualitiesRequest[] = [];
  openPlugin("radarr", {
    getPlugin: () => ({ plugin: radarr }),
    listQualities: () => ({ qualities }),
    testPluginSettings: () => ({ choices: { quality_profile: { choices: profiles } } }),
    replaceQualities: (request) => {
      sent.push(request);
      return replace(request);
    },
  });
  return sent;
}

/** The Qualities list on Radarr's page, once its service's profiles are known. */
async function qualitiesList() {
  const page = await plugin("Radarr");
  const list = within(await page.findByRole("list", { name: "Qualities" }));
  await list.findAllByText(/HD-1080p|Ultra-HD/u);
  return list;
}

/** Chooses `item` in the "…" of the Quality `name`. */
async function choose(name: string, item: string) {
  fireEvent.click(await screen.findByRole("button", { name: `More for ${name}` }));
  fireEvent.click(await screen.findByRole("menuitem", { name: item }));
}

/** The first list `sent` holds, once the Server got one: each Quality's id, profile, name and whether it's the default. */
async function firstSent(sent: ReplaceQualitiesRequest[]) {
  await waitFor(() => {
    expect(sent).not.toHaveLength(0);
  });
  expect(sent[0]?.id).toBe("radarr");
  return sent[0]?.qualities.map((quality) => [
    quality.id,
    quality.profileId,
    quality.name,
    quality.isDefault,
  ]);
}

test("a Fulfiller's Qualities are listed by name, with the default, their profile and description", async () => {
  openRadarr({ qualities: [HD, UHD] });

  const list = await qualitiesList();

  // What each row says, leaving out the dialog that asks before it's removed.
  const rows = list.getAllByRole("listitem").map((row) => row.firstElementChild?.textContent);
  expect(rows).toStrictEqual(["HDDefaultHD-1080p · About 8 GB per film", "4KUltra-HD"]);
});

test("the plus opens a row for a new Quality, which Add saves with the rest of the list", async () => {
  const sent = openRadarr();
  const list = await qualitiesList();

  fireEvent.click(screen.getByRole("button", { name: "Add a Quality" }));
  pick("Profile", "Ultra-HD");
  fireEvent.change(list.getByLabelText("Name"), { target: { value: "4K" } });
  // Nothing is sent as a new Quality is typed: only Add sends it.
  expect(sent).toStrictEqual([]);
  fireEvent.click(list.getByRole("button", { name: "Add" }));

  await expect(firstSent(sent)).resolves.toStrictEqual([
    [3n, "4", "HD", true],
    [0n, "5", "4K", false],
  ]);
  // Its row closes once it's saved.
  await waitFor(() => {
    expect(list.queryByLabelText("Name")).toBeNull();
  });
});

test("the first Quality added is the default, and Cancel adds none", async () => {
  const sent = openRadarr({ qualities: [] });
  const page = await plugin("Radarr");

  fireEvent.click(await page.findByRole("button", { name: "Add a Quality" }));
  fireEvent.click(page.getByRole("button", { name: "Cancel" }));
  expect(page.queryByLabelText("Name")).toBeNull();

  fireEvent.click(page.getByRole("button", { name: "Add a Quality" }));
  pick("Profile", "HD-1080p");
  fireEvent.change(page.getByLabelText("Name"), { target: { value: "HD" } });
  fireEvent.click(page.getByRole("button", { name: "Add" }));

  await expect(firstSent(sent)).resolves.toStrictEqual([[0n, "4", "HD", true]]);
});

test("edit opens a Quality's fields under it, and each saves as it's changed", async () => {
  const sent = openRadarr({ qualities: [HD, UHD] });
  const list = await qualitiesList();

  await choose("4K", "Edit");
  const name = await list.findByLabelText("Name");
  fireEvent.change(name, { target: { value: "Ultra" } });
  expect(sent).toStrictEqual([]);
  fireEvent.blur(name);

  await expect(firstSent(sent)).resolves.toStrictEqual([
    [3n, "4", "HD", true],
    [7n, "5", "Ultra", false],
  ]);
  expect(list.queryByRole("button", { name: "Save" })).toBeNull();

  fireEvent.click(list.getByRole("button", { name: "Done" }));
  expect(list.queryByLabelText("Name")).toBeNull();
});

test("make default saves the list with that Quality its only default", async () => {
  const sent = openRadarr({ qualities: [HD, UHD] });
  await qualitiesList();
  fireEvent.click(screen.getByRole("button", { name: "More for HD" }));
  await screen.findByRole("menuitem", { name: "Edit" });
  // The default has nothing to be made.
  expect(screen.queryByRole("menuitem", { name: "Make default" })).toBeNull();
  fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });

  await choose("4K", "Make default");

  await expect(firstSent(sent)).resolves.toStrictEqual([
    [3n, "4", "HD", false],
    [7n, "5", "4K", true],
  ]);
});

test("removing a Quality asks first, and says why the Server refused", async () => {
  const sent = openRadarr({
    qualities: [HD, UHD],
    replace: () => {
      throw refused(Code.InvalidArgument, "the list needs a default", {
        reason: RefusalReason.DEFAULT_QUALITY_REQUIRED,
        field: "qualities",
      });
    },
  });
  await qualitiesList();

  await choose("HD", "Remove");
  const dialog = within(await screen.findByRole("alertdialog", { name: "Remove HD?" }));
  expect(sent).toStrictEqual([]);
  fireEvent.click(dialog.getByRole("button", { name: "Remove" }));

  await expect(firstSent(sent)).resolves.toStrictEqual([[7n, "5", "4K", false]]);
  await expect(dialog.findByText("Make another Quality the default first.")).resolves.toBeDefined();
});

test("a Quality whose profile the service no longer offers is flagged", async () => {
  openRadarr({ profiles: [{ value: "5", label: "Ultra-HD" }] });
  const page = await plugin("Radarr");

  await expect(page.findByText("Radarr doesn’t offer this profile now.")).resolves.toBeDefined();
});

test("a name another Quality has is refused under the list", async () => {
  openRadarr({
    replace: () => {
      throw refused(Code.InvalidArgument, "two Qualities have the same name", {
        reason: RefusalReason.QUALITY_REPEATED,
        field: "qualities[0].name",
      });
    },
  });
  const list = await qualitiesList();

  await choose("HD", "Edit");
  const name = await list.findByLabelText("Name");
  fireEvent.change(name, { target: { value: "4K" } });
  fireEvent.blur(name);

  const page = await plugin("Radarr");
  await expect(page.findByText("Another Quality has this name.")).resolves.toBeDefined();
});

test("while the service's profiles can't be fetched, the list stays and nothing is added or edited", async () => {
  openPlugin("radarr", {
    getPlugin: () => ({ plugin: radarr }),
    listQualities: () => ({ qualities: [HD] }),
    testPluginSettings: () => {
      throw refused(Code.FailedPrecondition, "The setting url is required.", {
        reason: RefusalReason.SETTINGS_CHECK_FAILED,
        field: "settings.url",
      });
    },
  });
  const page = await plugin("Radarr");

  await page.findByText(/^Couldn’t get Radarr’s quality profiles: /u);
  expect(page.getByText("HD")).toBeDefined();
  expect(page.queryByRole("button", { name: "Add a Quality" })).toBeNull();
  fireEvent.click(page.getByRole("button", { name: "More for HD" }));
  const edit = await screen.findByRole("menuitem", { name: "Edit" });
  expect(edit.getAttribute("aria-disabled")).toBe("true");
});

test("a Plugin without a Quality profile setting has no Qualities", async () => {
  openPlugin("test-choices");

  await plugin("test-choices");

  expect(screen.queryByRole("heading", { name: "Qualities" })).toBeNull();
});

test("a Fulfiller for Shows has Qualities too", async () => {
  const sonarr = {
    ...radarr,
    summary: {
      ...radarr.summary,
      id: "sonarr",
      name: "Sonarr",
      extensionPoints: [ExtensionPoint.SHOW_FULFILLER],
    },
  };
  openPlugin("sonarr", {
    getPlugin: () => ({ plugin: sonarr }),
    listQualities: () => ({}),
    testPluginSettings: () => ({}),
  });

  await plugin("Sonarr");

  await expect(screen.findByRole("heading", { name: "Qualities" })).resolves.toBeDefined();
});
