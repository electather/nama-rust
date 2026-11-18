import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import {
  BrowseService,
  type ListMoviesRequest,
  MovieSort,
  WatchedFilter,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const LIBRARY_SIZE = 1000;

/**
 * A fake Server whose Movies Library (1) holds 1,000 Movies, listed 100 a page with the index of
 * the first Movie of each page as its token. Each `ListMovies` it gets goes in `asked`.
 */
function thousandMovies(asked: ListMoviesRequest[]) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listUnmatched: () => ({}),
      listGenres: () => ({ genres: ["Crime", "Horror"] }),
      listMovies: (request) => {
        asked.push(request);
        const first = Number(request.page?.token ?? "") || 0;
        const size = request.page?.size ?? 50;
        const count = Math.min(size, LIBRARY_SIZE - first);
        const movies = Array.from({ length: count }, (_, at) => {
          const id = String(first + at + 1);
          return { id: { namespace: "tmdb", value: id }, title: `Movie ${id}` };
        });
        const next = first + count < LIBRARY_SIZE ? String(first + count) : "";
        const total = first === 0 ? { totalSize: LIBRARY_SIZE } : {};
        return { movies, page: { nextToken: next, ...total } };
      },
    });
  });
}

/** Stands in for the browser's `IntersectionObserver`, with the end of the grid always in view. */
function endAlwaysInView() {
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      seen: (entries: { isIntersecting: boolean }[]) => void;
      constructor(seen: (entries: { isIntersecting: boolean }[]) => void) {
        this.seen = seen;
      }
      observe() {
        this.seen([{ isIntersecting: true }]);
      }
      disconnect() {}
    },
  );
}

test("the grid loads a 1,000-title Library page by page, keeping only the rows near the screen", async () => {
  endAlwaysInView();
  // Laid out, every poster's row is 400px tall, so a 768px window shows a few rows. Without a
  // ResizeObserver the grid reads each row's `offsetHeight`; happy-dom's would measure 0px.
  // oxlint-disable-next-line unicorn/no-useless-undefined -- `stubGlobal` takes the value to stub, here none
  vi.stubGlobal("ResizeObserver", undefined);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(400);
  const asked: ListMoviesRequest[] = [];
  openApp("/libraries/1", thousandMovies(asked));

  expect(await screen.findByText("1,000 Movies")).toBeDefined();
  await waitFor(() => {
    expect(asked).toHaveLength(LIBRARY_SIZE / 100);
  });

  expect(asked.map(({ page }) => page?.size)).toEqual(Array.from({ length: 10 }, () => 100));
  expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
  expect(document.querySelectorAll("li[aria-posinset]").length).toBeLessThan(100);
});

test("choosing a sort or a filter asks for the first page of it", async () => {
  const asked: ListMoviesRequest[] = [];
  openApp("/libraries/1", thousandMovies(asked));
  await screen.findByText("1,000 Movies");

  fireEvent.click(screen.getByRole("button", { name: "Sort: Title" }));
  fireEvent.click(await screen.findByRole("menuitemradio", { name: "Recently Added" }));
  await screen.findByRole("button", { name: "Sort: Recently Added" });
  fireEvent.click(screen.getByRole("button", { name: "Watch State: All" }));
  fireEvent.click(await screen.findByRole("menuitemradio", { name: "Not started" }));
  await screen.findByRole("button", { name: "Watch State: Not started" });
  act(() => {
    fireEvent.change(screen.getByRole("spinbutton", { name: "From year" }), {
      target: { value: "1990" },
    });
  });

  await waitFor(() => {
    expect(asked.at(-1)?.filter?.minYear).toBe(1990);
  });
  const last = asked.at(-1);
  expect(last?.sort).toBe(MovieSort.ADDED);
  expect(last?.direction).toBe(SortDirection.DESCENDING);
  expect(last?.filter?.watched).toBe(WatchedFilter.NOT_STARTED);
  expect(last?.page?.token).toBe("");
});

test("choosing two genres asks for the titles with either", async () => {
  const asked: ListMoviesRequest[] = [];
  openApp("/libraries/1", thousandMovies(asked));
  await screen.findByText("1,000 Movies");

  fireEvent.click(screen.getByRole("button", { name: "Genre: All" }));
  fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Crime" }));
  await screen.findByRole("button", { name: "Genre: Crime" });
  fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Horror" }));
  await screen.findByRole("button", { name: "Genre: Crime +1" });

  await waitFor(() => {
    expect(asked.at(-1)?.filter?.genres).toEqual(["Crime", "Horror"]);
  });
  expect(asked.at(-1)?.page?.token).toBe("");
});
