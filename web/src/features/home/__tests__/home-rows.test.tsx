import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService, type ListHomeRowTitlesRequest } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { RequestState } from "@/gen/nama/v1/request_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { type HomeRowInit, continueWatchingRow, pluginRow } from "@/test-home-rows";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const heat = { libraryId: 1n, id: { namespace: "tmdb", value: "949" }, title: "Heat" };
const dark = { libraryId: 2n, id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
const dune = {
  id: { namespace: "tmdb", value: "438631" },
  title: "Dune",
  year: 2021,
  posterUrl: "/media/artwork/tmdb/%2Fdune.jpg",
  request: { id: 7n, state: RequestState.PENDING, requesters: [{ userId: 9n, name: "Ada" }] },
};
const arrival = {
  id: { namespace: "tmdb", value: "329865" },
  title: "Arrival",
  request: { id: 8n, state: RequestState.DECLINED },
};
const alien = {
  libraryId: 1n,
  movieOrEpisode: {
    case: "movie",
    value: {
      id: { namespace: "tmdb", value: "348" },
      title: "Alien",
      watchState: { positionMs: 1n, durationMs: 60_000n },
    },
  },
} as const;
const severance = { id: { namespace: "tmdb", value: "95396" }, title: "Severance", year: 2022 };

const picksRow = pluginRow("lists", "picks", "Picks");
const emptyRow = pluginRow("lists", "empty", "Nothing here");
const showsRow = pluginRow("trakt", "shows", "Trending Shows");

/**
 * Home on a Server that lists an empty Continue Watching, then the Plugin rows `picks`, `empty`
 * and `shows`. Given `rows`, it lists those instead, with Alien in Continue Watching.
 */
function openHome(asked: ListHomeRowTitlesRequest[] = [], rows?: HomeRowInit[]) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listFeatured: () => ({}),
      listContinueWatching: () => (rows ? { entries: [alien] } : {}),
      listHomeRows: () => ({ rows: rows ?? [continueWatchingRow, picksRow, emptyRow, showsRow] }),
      listHomeRowTitles: (request) => {
        asked.push(request);
        if (request.key === "picks") {
          return {
            titles: [
              { movieOrShow: { case: "movie", value: heat } },
              { movieOrShow: { case: "missingMovie", value: dune } },
              { movieOrShow: { case: "missingMovie", value: arrival } },
              { movieOrShow: { case: "missingShow", value: severance } },
            ],
          };
        }
        if (request.key === "shows") {
          return { titles: [{ movieOrShow: { case: "show", value: dark } }] };
        }
        return {};
      },
    });
  });
  openApp("/", transport);
}

/**
 * Stands in for the browser's `IntersectionObserver`: every element it watches is near the view
 * once `scrolled` is called, and from then on.
 */
function scrollable() {
  const watching: ((entries: { isIntersecting: boolean }[]) => void)[] = [];
  let near = false;
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      public seen: (entries: { isIntersecting: boolean }[]) => void;
      public constructor(seen: (entries: { isIntersecting: boolean }[]) => void) {
        this.seen = seen;
      }
      public observe() {
        watching.push(this.seen);
        this.seen([{ isIntersecting: near }]);
      }
      // oxlint-disable-next-line class-methods-use-this -- the stand-in watches nothing, so it has nothing to stop
      public disconnect() {}
    },
  );
  return {
    scrolled: () => {
      near = true;
      for (const seen of watching) {
        seen([{ isIntersecting: true }]);
      }
    },
  };
}

test("home shows its Libraries, then each Plugin's row in order, each poster opening its title", async () => {
  scrollable().scrolled();
  openHome();

  // A row shows its skeleton under its heading until its titles arrive.
  await screen.findByRole("link", { name: "Dark" });
  await waitFor(() => {
    const headings = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(headings).toStrictEqual(["Libraries", "Picks", "Trending Shows"]);
  });
  const shows = screen.getByRole("region", { name: "Trending Shows" });
  const picks = screen.getByRole("region", { name: "Picks" });
  expect(within(picks).getByRole("link", { name: "Heat" }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-949",
  );
  expect(within(shows).getByRole("link", { name: "Dark" }).getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-70523",
  );
});

test("home draws Continue Watching where the Server lists it among the Plugins' rows", async () => {
  scrollable().scrolled();
  openHome([], [picksRow, continueWatchingRow, showsRow]);

  await screen.findByRole("link", { name: "Dark" });
  await screen.findByRole("link", { name: /Alien/u });
  const headings = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
  expect(headings).toStrictEqual(["Libraries", "Picks", "Continue Watching", "Trending Shows"]);
});

test("only the first Plugin row loads at once when Continue Watching sits below it", async () => {
  scrollable();
  const asked: ListHomeRowTitlesRequest[] = [];
  openHome(asked, [picksRow, showsRow, continueWatchingRow]);

  await screen.findByRole("link", { name: "Heat" });
  await screen.findByRole("link", { name: /Alien/u });
  expect(asked.map((request) => request.key)).toStrictEqual(["picks"]);
});

test("the first Plugin row loads at once, and each below it only as you scroll near", async () => {
  const { scrolled } = scrollable();
  const asked: ListHomeRowTitlesRequest[] = [];
  openHome(asked);

  await screen.findByRole("link", { name: "Heat" });
  expect(asked.map((request) => request.key)).toStrictEqual(["picks"]);

  scrolled();
  await screen.findByRole("link", { name: "Dark" });
  await waitFor(() => {
    expect(screen.queryByRole("region", { name: "Nothing here" })).toBeNull();
  });
  expect(asked.map((request) => [request.key, request.page?.size])).toStrictEqual([
    ["picks", 12],
    ["empty", 12],
    ["shows", 12],
  ]);
});

test("a missing title opens its page to Request it, badged with where it stands", async () => {
  scrollable().scrolled();
  openHome();

  await screen.findByRole("link", { name: "Dune" });
  const picks = screen.getByRole("region", { name: "Picks" });

  const dunePoster = within(picks).getByRole("link", { name: "Dune" });
  expect(dunePoster.getAttribute("href")).toBe("/movies/tmdb-438631");
  expect(dunePoster.querySelector("img:not([src^='data:'])")?.getAttribute("src")).toBe(
    dune.posterUrl,
  );
  expect(within(dunePoster).getByText("Pending")).toBeDefined();
  // A declined Request leaves the title as missing as one nobody asked for.
  const arrivalPoster = within(picks).getByRole("link", { name: "Arrival" });
  expect(within(arrivalPoster).getByText("Not on this Server")).toBeDefined();
  const severancePoster = within(picks).getByRole("link", { name: "Severance" });
  expect(severancePoster.getAttribute("href")).toBe("/shows/tmdb-95396");
  expect(within(severancePoster).getByText("Not on this Server")).toBeDefined();
  // A title you can watch carries no badge.
  expect(within(picks).getByRole("link", { name: "Heat" }).textContent).toBe("");
});

/** A Server whose one row has 13 titles, 12 to a page, each page's token noted in `asked`. */
function aRowOfThirteen(asked: (string | undefined)[]) {
  const titles = Array.from(
    { length: 12 },
    (_, index) =>
      ({
        movieOrShow: {
          case: "movie",
          value: { ...heat, id: { namespace: "tmdb", value: String(index) } },
        },
      }) as const,
  );
  return fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(BrowseService, {
      listFeatured: () => ({}),
      listContinueWatching: () => ({}),
      listHomeRows: () => ({ rows: [picksRow] }),
      listHomeRowTitles: ({ page }) => {
        asked.push(page?.token);
        return page?.token === "" ? { titles, page: { nextToken: "12", totalSize: 13 } } : {};
      },
    });
  });
}

test("a row's shelf asks for its next page as you scroll to its end", async () => {
  scrollable().scrolled();
  // Laid out, a page of 12 posters fits across the shelf at once, so it's at its end.
  const asked: (string | undefined)[] = [];
  openApp("/", aRowOfThirteen(asked));

  await waitFor(() => {
    expect(asked).toStrictEqual(["", "12"]);
  });
});
