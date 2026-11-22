import { describe, expect, test } from "vite-plus/test";

import { jobSearch } from "@/shared/lib/job-key";
import { settingsDetailParams } from "@/shared/lib/settings-detail-params";

describe("opening a Job in Settings", () => {
  test("drops every other detail already in the address", () => {
    const address = { plugin: "x", user: 2, job: "y", library: "z", setting: "s" };
    const search = { ...address, ...jobSearch("p", "j") };

    expect(search).toStrictEqual({ ...settingsDetailParams, settings: "jobs", job: "p:j" });
  });
});
