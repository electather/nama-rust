import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import {
  BrowseService,
  type ListMoviesRequest,
  type ListShowsRequest,
  MovieSort,
  ShowSort,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { recentlyAddedRow } from "@/test-home-rows";
import { layOutShelves } from "@/test-layout";

beforeEach(() => {
  layOutShelves();
  // Every row is near the view, so each loads.
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      public seen: (entries: { isIntersecting: boolean }[]) => void;
      public constructor(seen: (entries: { isIntersecting: boolean }[]) => void) {
        this.seen = seen;
      }
      public observe() {
        this.seen([{ isIntersecting: true }]);
      }
      // oxlint-disable-next-line class-methods-use-this -- the stand-in watches nothing, so it has nothing to stop
      public disconnect() {}
    },
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

/** Home on a Server whose rows are the Recently Added rows of Films, a Movies Library, and TV. */
function openHome(asked: (ListMoviesRequest | ListShowsRequest)[]) {
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(BrowseService, {
      listFeatured: () => ({}),
      listContinueWatching: () => ({}),
      listHomeRows: () => ({
        rows: [
          recentlyAddedRow(1n, "Films", LibraryKind.MOVIES),
          recentlyAddedRow(2n, "TV", LibraryKind.SHOWS),
        ],
      }),
      listMovies: (request) => {
        asked.push(request);
        // More to come, which the row leaves to the Library page.
        return {
          movies: [{ id: { namespace: "tmdb", value: "949" }, title: "Heat" }],
          page: { nextToken: "more", totalSize: 21 },
        };
      },
      listShows: (request) => {
        asked.push(request);
        return { shows: [{ id: { namespace: "tmdb", value: "70523" }, title: "Dark" }] };
      },
    });
  });
  openApp("/", transport);
}

test("each Library's Recently Added row lists its 20 newest titles, its heading opening the Library", async () => {
  const asked: (ListMoviesRequest | ListShowsRequest)[] = [];
  openHome(asked);

  await screen.findByRole("link", { name: "Heat" });
  await screen.findByRole("link", { name: "Dark" });
  const films = screen.getByRole("region", { name: "Recently added in Films" });
  const tv = screen.getByRole("region", { name: "Recently added in TV" });
  expect(
    within(films).getByRole("link", { name: "Recently added in Films" }).getAttribute("href"),
  ).toBe("/libraries/1?sort=added");
  expect(within(films).getByRole("link", { name: "Heat" }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-949",
  );
  expect(within(tv).getByRole("link", { name: "Dark" }).getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-70523",
  );
  expect(
    asked.map(({ libraryId, sort, direction, page }) => [libraryId, sort, direction, page?.size]),
  ).toStrictEqual([
    [1n, MovieSort.ADDED, SortDirection.DESCENDING, 20],
    [2n, ShowSort.ADDED, SortDirection.DESCENDING, 20],
  ]);
});
