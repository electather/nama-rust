import { describe, expect, test } from "vite-plus/test";

import { sizeOf } from "@/shared/lib/title-facts";

describe("a file's size", () => {
  test.each([
    [500, "0.5 kB"],
    [999_000, "999 kB"],
    [999_950, "1 MB"],
    [15_200_000, "15.2 MB"],
    [48_900_000, "48.9 MB"],
    [98_000_000, "98 MB"],
    [999_940_000, "999.9 MB"],
    [999_960_000, "1 GB"],
    [1_000_000_000, "1 GB"],
    [1_400_000_000, "1.4 GB"],
    [71_400_000_000, "71.4 GB"],
  ])("%i bytes reads %s", (bytes, shown) => {
    expect(sizeOf(BigInt(bytes))).toBe(shown);
  });
});
