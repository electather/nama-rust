import { describe, expect, test } from "vite-plus/test";

import { sizeOf } from "@/shared/lib/title-facts";

describe("a file's size", () => {
  test.each([
    [500n, "0.5 kB"],
    [999_000n, "999 kB"],
    [999_950n, "1 MB"],
    [15_200_000n, "15.2 MB"],
    [48_900_000n, "48.9 MB"],
    [98_000_000n, "98 MB"],
    [999_940_000n, "999.9 MB"],
    [999_960_000n, "1 GB"],
    [1_000_000_000n, "1 GB"],
    [1_400_000_000n, "1.4 GB"],
    [71_400_000_000n, "71.4 GB"],
  ])("%i bytes reads %s", (bytes, shown) => {
    expect(sizeOf(bytes)).toBe(shown);
  });
});
