import { describe, expect, test } from "vite-plus/test";

import { closedSearch } from "@/features/settings/lib/sections";
import { settingsDetailParams } from "@/shared/lib/settings-detail-params";

describe("closing Settings", () => {
  test("drops the section and every detail in the address", () => {
    const address = {
      plugin: "x",
      user: 2,
      job: "y",
      library: "z",
      setting: "s",
      settings: "jobs",
    };
    const search = { ...address, ...closedSearch };

    expect(search).toStrictEqual({ ...settingsDetailParams, settings: undefined });
  });
});
