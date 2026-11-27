import { describe, expect, test } from "vite-plus/test";

import {
  type Section,
  type SettingsDetail,
  type SettingsPage,
  allSections,
  closedSearch,
  detailSearch,
  parseSettings,
  pickedServerSearch,
  sectionSearch,
  sections,
  settingsSearch,
} from "@/shared/lib/settings-address";

describe("the Settings address", () => {
  // Every word set, so a build that leaves one behind shows.
  const everyWord = {
    search: "dune",
    settings: "jobs",
    manage: "cabin",
    plugin: "tmdb",
    user: 2,
    job: "upkeep:back-up",
    library: "3",
    setting: "schedule",
  };

  test.each<SettingsPage>([...sections, allSections])(
    "opening %s keeps only it and the picked Server",
    (page) => {
      expect(parseSettings({ ...everyWord, ...sectionSearch(page) })).toStrictEqual({
        page,
        detail: undefined,
        setting: undefined,
        server: "cabin",
      });
    },
  );

  test.each<[SettingsDetail, Section]>([
    [{ kind: "plugin", id: "tmdb" }, "plugins"],
    [{ kind: "user", id: 7 }, "users"],
    [{ kind: "job", pluginId: "upkeep", jobId: "back-up" }, "jobs"],
    [{ kind: "library", id: "4" }, "libraries"],
    [{ kind: "library", id: "new" }, "libraries"],
    [{ kind: "library-filter", id: "4" }, "needs-a-match"],
  ])("opening %o opens %s with no other detail", (detail, page) => {
    expect(parseSettings({ ...everyWord, ...detailSearch(detail) })).toStrictEqual({
      page,
      detail,
      setting: undefined,
      server: "cabin",
    });
  });

  test("opening a Section at a Setting holds the Setting", () => {
    expect(parseSettings({ ...everyWord, ...sectionSearch("server", "store") })).toStrictEqual({
      page: "server",
      detail: undefined,
      setting: "store",
      server: "cabin",
    });
  });

  test.each([
    ["another Server", "attic"],
    ["the Account", undefined],
  ])("picking %s keeps the Section and drops the last one's details", (_, server) => {
    expect(parseSettings({ ...everyWord, ...pickedServerSearch(server) })).toStrictEqual({
      page: "jobs",
      detail: undefined,
      setting: undefined,
      server,
    });
  });

  test("closing empties every Settings word, the picked Server's too, and keeps the rest", () => {
    expect({ ...everyWord, ...closedSearch }).toStrictEqual({
      search: "dune",
      settings: undefined,
      manage: undefined,
      plugin: undefined,
      user: undefined,
      job: undefined,
      library: undefined,
      setting: undefined,
    });
  });

  test("the route reads back only the words the address holds", () => {
    expect(settingsSearch(everyWord)).toStrictEqual({
      settings: "jobs",
      manage: "cabin",
      plugin: undefined,
      user: undefined,
      job: "upkeep:back-up",
      library: undefined,
      setting: "schedule",
    });
  });

  test("an unknown Section opens Playback", () => {
    expect(parseSettings({ settings: "nope" }).page).toBe("playback");
  });

  test.each([
    ["a detail its Section doesn't open", { settings: "users", plugin: "tmdb" }],
    ["a detail with all of Settings open", { settings: "all", plugin: "tmdb" }],
    ["a User that isn't a whole number", { settings: "users", user: "2.5" }],
    ["a User that's no number", { settings: "users", user: "sam" }],
    ["an empty User", { settings: "users", user: "" }],
    ["a Job without its Plugin", { settings: "jobs", job: "back-up" }],
    ["an id that's no word", { settings: "plugins", plugin: ["tmdb"] }],
  ])("%s opens no detail", (_, search) => {
    expect(parseSettings(search).detail).toBeUndefined();
  });

  test("every word is dropped while Settings is closed", () => {
    expect(parseSettings({ ...everyWord, settings: undefined })).toStrictEqual({
      page: undefined,
      detail: undefined,
      setting: undefined,
      server: undefined,
    });
  });
});
