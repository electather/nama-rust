import { create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import type { TitleListing } from "@/features/library/lib/title-listing";
import type { Paged } from "@/features/library/lib/titles-props";
import { withUnmatched } from "@/features/library/lib/with-unmatched";
import { UnmatchedItemSummarySchema } from "@/gen/nama/v1/browse_pb";

const alien = create(UnmatchedItemSummarySchema, { itemId: 5n, title: "Alien" });

/** The Library's titles: `pages` pages, 50 in all, whose `fetchNextPage` adds `name` to `asked`. */
function titlesAsking(asked: string[], more: { pages: number; hasNextPage: boolean }) {
  return {
    data: { pages: Array.from({ length: more.pages }, () => ({})) },
    hasNextPage: more.hasNextPage,
    totalSize: 50,
    fetchNextPage: async () => {
      asked.push("titles");
      await Promise.resolve();
    },
  } satisfies Paged<unknown>;
}

/** The Library's Unmatched Items: one page of Alien, 3 in all. */
function unmatchedAsking(asked: string[], hasNextPage: boolean) {
  return {
    data: { pages: [{ items: [alien] }] },
    hasNextPage,
    totalSize: 3,
    fetchNextPage: async () => {
      asked.push("unmatched");
      await Promise.resolve();
    },
  } satisfies Paged<{ items: (typeof alien)[] }>;
}

describe("a Library's titles joined with its Unmatched Items", () => {
  test.each([
    {
      case: "the titles are all loaded: tiles follow them",
      listing: {} satisfies TitleListing,
      titlesNext: false,
      expected: { tiles: 1, only: false, hasNextPage: false, totalSize: 53, pages: 3 },
    },
    {
      case: "a titles page is still to load: no tiles yet, and more to come",
      listing: {} satisfies TitleListing,
      titlesNext: true,
      expected: { tiles: 0, only: false, hasNextPage: true, totalSize: 53, pages: 3 },
    },
    {
      case: "a title filter is on: no tiles, and only the titles count",
      listing: { genres: ["Crime"] } satisfies TitleListing,
      titlesNext: false,
      expected: { tiles: 0, only: false, hasNextPage: false, totalSize: 50, pages: 2 },
    },
    {
      case: "Not matched is on: only the tiles, counted as they are",
      listing: { unmatched: true } satisfies TitleListing,
      titlesNext: true,
      expected: { tiles: 1, only: true, hasNextPage: false, totalSize: 3, pages: 1 },
    },
  ])("$case", ({ listing, titlesNext, expected }) => {
    const titles = titlesAsking([], { pages: 2, hasNextPage: titlesNext });

    const joined = withUnmatched(titles, unmatchedAsking([], false), listing);

    expect({
      tiles: joined.items.length,
      only: joined.only,
      hasNextPage: joined.paged.hasNextPage,
      totalSize: joined.paged.totalSize,
      pages: joined.paged.data.pages.length,
    }).toStrictEqual(expected);
  });

  test.each([
    { case: "titles first while they have more", titlesNext: true, expected: ["titles"] },
    { case: "the Unmatched Items after them", titlesNext: false, expected: ["unmatched"] },
  ])("pages $case", async ({ titlesNext, expected }) => {
    const asked: string[] = [];
    const titles = titlesAsking(asked, { pages: 1, hasNextPage: titlesNext });

    await withUnmatched(titles, unmatchedAsking(asked, true), {}).paged.fetchNextPage();

    expect(asked).toStrictEqual(expected);
  });

  test("has more while only the Unmatched Items do", () => {
    const titles = titlesAsking([], { pages: 1, hasNextPage: false });

    expect(withUnmatched(titles, unmatchedAsking([], true), {}).paged.hasNextPage).toBe(true);
  });
});
