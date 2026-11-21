import { describe, expect, test } from "vite-plus/test";

import { duration } from "@/features/admin-jobs/lib/run-times";

describe("how long a Run took", () => {
  test.each([
    [400n, "Under a second"],
    [75_000n, "1 minute 15 seconds"],
    [120_000n, "2 minutes"],
    [5_400_000n, "1 hour 30 minutes"],
  ])("%s ms reads %s", (ms, words) => {
    expect(duration(0n, ms)).toBe(words);
  });
});
