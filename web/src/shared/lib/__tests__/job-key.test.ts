import { describe, expect, test } from "vite-plus/test";

import { jobSearch } from "@/shared/lib/job-key";

describe("opening a Job in Settings", () => {
  test("drops a Library already in the address", () => {
    const address = { library: "x" };
    const search = { ...address, ...jobSearch("p", "j") };

    expect(search.library).toBeUndefined();
    expect(search.settings).toBe("jobs");
    expect(search.job).toBe("p:j");
  });
});
