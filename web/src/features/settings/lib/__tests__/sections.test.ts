import { describe, expect, test } from "vite-plus/test";

import { closedSearch } from "@/features/settings/lib/sections";
import { settingsDetailParams } from "@/shared/lib/settings-detail-params";

describe("closing Settings", () => {
  test("drops the section, every detail and the Server picked in the address", () => {
    const details = Object.keys(settingsDetailParams).map((key): [string, string] => [key, "x"]);
    const address = { ...Object.fromEntries(details), settings: "jobs", manage: "cabin" };
    const search = { ...address, ...closedSearch };

    expect(search).toStrictEqual({
      ...settingsDetailParams,
      settings: undefined,
      manage: undefined,
    });
  });
});
