import { describe, expect, test } from "vite-plus/test";

import {
  filterLabel,
  filterNames,
  listingFromSearch,
  unfiltered,
  withFilter,
} from "@/features/library/lib/title-listing";

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

  test("name Not matched last, and clearing drops it", () => {
    const listing = { unmatched: true as const, sort: "added" as const };

    expect([filterLabel(listing), unfiltered(listing)]).toStrictEqual([
      "Filter: Not matched",
      { sort: "added", order: undefined },
    ]);
  });

  test("turn Not matched off when a title filter changes", () => {
    const listing = { unmatched: true as const };

    expect(withFilter(listing, { genres: ["Crime"] })).toStrictEqual({
      genres: ["Crime"],
      unmatched: undefined,
    });
  });

  test.each([
    { search: { unmatched: 1 }, unmatched: true },
    { search: { unmatched: true }, unmatched: true },
    { search: { unmatched: "yes" }, unmatched: undefined },
    { search: {}, unmatched: undefined },
  ])("read Not matched from $search", ({ search, unmatched }) => {
    expect(listingFromSearch(search).unmatched).toBe(unmatched);
  });
});
