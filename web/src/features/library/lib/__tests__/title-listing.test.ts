import { describe, expect, test } from "vite-plus/test";

import { filterLabel, filterNames, unfiltered } from "@/features/library/lib/title-listing";

describe("a listing's filters", () => {
  test("are named genres first, then the years, then the Watch State", () => {
    const listing = { genres: ["Crime", "Horror"], from: 1990, watched: "played" as const };

    expect(filterNames(listing)).toStrictEqual(["Crime", "Horror", "From 1990", "Watched"]);
  });

  test.each([
    { from: 1990, to: 1999, name: "1990–1999" },
    { from: 1990, to: undefined, name: "From 1990" },
    { from: undefined, to: 1999, name: "To 1999" },
  ])("name the years $name as one", ({ from, to, name }) => {
    expect(filterNames({ from, to })).toStrictEqual([name]);
  });

  test.each([
    { listing: {}, label: "Filter" },
    { listing: { watched: "not-started" as const }, label: "Filter: Not started" },
    { listing: { genres: ["Drama"], watched: "started" as const }, label: "Filter: Drama +1" },
  ])("put the first on the Filter button and count the rest: $label", ({ listing, label }) => {
    expect(filterLabel(listing)).toBe(label);
  });

  test("clear keeping the sort and its direction", () => {
    const listing = { sort: "released" as const, order: "asc" as const, genres: ["Crime"] };

    expect(unfiltered(listing)).toStrictEqual({ sort: "released", order: "asc" });
  });
});
