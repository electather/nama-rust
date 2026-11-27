import { describe, expect, test } from "vite-plus/test";

import { filterCount, filterTokens, unfiltered } from "@/features/library/lib/title-listing";

describe("a listing's filters", () => {
  test("each have a chip that turns that one off", () => {
    const listing = {
      sort: "added" as const,
      genres: ["Crime", "Horror"],
      watched: "played" as const,
    };

    const tokens = filterTokens(listing);

    expect(tokens.map(({ name }) => name)).toStrictEqual(["Crime", "Horror", "Watched"]);
    expect(tokens[0]?.without).toStrictEqual({ ...listing, genres: ["Horror"] });
    expect(tokens[2]?.without).toStrictEqual({ ...listing, watched: undefined });
  });

  test.each([
    { from: 1990, to: 1999, name: "1990–1999" },
    { from: 1990, to: undefined, name: "From 1990" },
    { from: undefined, to: 1999, name: "To 1999" },
  ])("name the years $name in one chip", ({ from, to, name }) => {
    expect(filterTokens({ from, to }).map((token) => token.name)).toStrictEqual([name]);
  });

  test("count each genre, the years once and the Watch State, and clear keeping the sort", () => {
    const listing = {
      sort: "released" as const,
      order: "asc" as const,
      genres: ["Crime"],
      from: 1990,
      to: 1999,
    };

    expect(filterCount(listing)).toBe(2);
    expect(unfiltered(listing)).toStrictEqual({ sort: "released", order: "asc" });
  });
});
