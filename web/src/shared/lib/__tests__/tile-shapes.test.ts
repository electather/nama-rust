import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { backdropTile, tileGap } from "@/shared/lib/tile-shapes";
import { openApp } from "@/test-app";
import { continueWatchingRow } from "@/test-home-rows";
import { layOutShelves } from "@/test-layout";
import { fakeServer, newUser } from "@/test-server";

/** Continue Watching entries for the Movies `M0` to `M<count - 1>`, each started. */
function started(count: number) {
  return Array.from({ length: count }, (_, at) => ({
    libraryId: 1n,
    movieOrEpisode: {
      case: "movie" as const,
      value: {
        id: { namespace: "tmdb", value: `m${at}` },
        title: `M${at}`,
        watchState: { positionMs: 1n, durationMs: 60_000n },
      },
    },
  }));
}

/** Home on a Server whose only row is Continue Watching, listing `entries` once they're there. */
function openHome(entries: Promise<ReturnType<typeof started>>) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const { transport } = fakeServer(
    { signedIn: newUser, libraries },
    {
      BrowseService: {
        listHomeRows: () => ({ rows: [continueWatchingRow] }),
        listContinueWatching: async () => ({ entries: await entries }),
      },
    },
  );
  openApp("/", transport);
}

/** A Show in the TV Library with `count` Episodes in its one Season. */
function openShow(count: number) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  const episodes = Array.from({ length: count }, (_, at) => ({
    seasonNumber: 1,
    episodeNumber: at + 1,
    title: `E${at + 1}`,
    versions: [{ id: 1n }],
  }));
  const { transport } = fakeServer(
    { signedIn: newUser, libraries },
    {
      BrowseService: {
        getShow: () => ({ show: { title: "Severance" }, seasons: [{ number: 1 }] }),
        listEpisodes: () => ({ episodes }),
      },
    },
  );
  openApp("/libraries/2/shows/tmdb-95396", transport);
}

/** The width `element`'s style gives it. */
function drawnWidth(element: HTMLElement | null) {
  return element?.style.getPropertyValue("--tile-width");
}

/** The `rounded-*` classes of `element`, its corners. */
function cornersOf(element: Element | null) {
  return [...(element?.classList ?? [])].filter((name) => name.startsWith("rounded"));
}

/** The width a row holds for the cards after those it renders, from its list's padding. */
function heldAfter(list: HTMLElement) {
  return list.style.getPropertyValue("--tiles-after");
}

describe("tile shapes", () => {
  beforeEach(layOutShelves);

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  test("continue Watching's skeleton is as wide and round as the cards that replace it", async () => {
    const answer: { with?: (entries: ReturnType<typeof started>) => void } = {};
    openHome(
      new Promise((resolve) => {
        answer.with = resolve;
      }),
    );
    const region = await screen.findByRole("region", { name: "Continue Watching" });
    const skeleton = await waitFor(() => {
      const box = region.querySelector<HTMLElement>(".animate-pulse");
      expect(box).not.toBeNull();
      return box;
    });
    const skeletonWidth = drawnWidth(skeleton);
    const skeletonCorners = cornersOf(skeleton);

    answer.with?.(started(1));

    // Continue Watching's section that loads is not the one that waits.
    const card = await screen.findByRole("link", { name: /^Resume M0/u });
    expect(skeletonWidth).not.toBe("");
    expect(drawnWidth(card.closest("li"))).toBe(skeletonWidth);
    expect(cornersOf(card.parentElement)).toStrictEqual(skeletonCorners);
  });

  test("a Home row holds a card and a gap for each card it doesn't render", async () => {
    openHome(Promise.resolve(started(40)));
    const region = await screen.findByRole("region", { name: "Continue Watching" });
    await within(region).findByRole("link", { name: /^Resume M0/u });
    const list = within(region).getByRole("list");
    const unrendered = 40 - within(list).getAllByRole("listitem").length;

    expect(unrendered).toBeGreaterThan(0);
    // A Home row takes a card it hasn't rendered to be as wide as its probe, which happy-dom never
    // lays out, so its shape's widest.
    expect(heldAfter(list)).toBe(`${unrendered * (backdropTile.widest + tileGap.px)}px`);
  });

  test("a Show's Episode row holds a card and a gap for each card it doesn't render", async () => {
    openShow(40);
    await screen.findByText("E1");
    const list = screen.getByRole("list", { name: "Episodes" });
    const unrendered = 40 - within(list).getAllByRole("listitem").length;

    expect(unrendered).toBeGreaterThan(0);
    // An Episode row takes every card to be as wide as one it renders, 100px in `layOutShelves`.
    expect(heldAfter(list)).toBe(`${unrendered * (100 + tileGap.px)}px`);
  });
});
