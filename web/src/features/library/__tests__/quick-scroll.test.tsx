import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type ListMoviesRequest, MovieSort } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const LIBRARY_SIZE = 600;

/**
 * A fake Server whose Movies Library (1) holds 600 Movies, 100 a page, the first starting with A
 * and those from the 450th with M, as its first page's index says. Each `ListMovies` goes in
 * `asked`.
 */
function sixHundredMovies(asked: ListMoviesRequest[]) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listUnmatched: () => ({}),
      listGenres: () => ({}),
      listMovies: (request) => {
        asked.push(request);
        const token = request.page?.token ?? "";
        const first = token === "" ? 0 : Number(token);
        const count = Math.min(100, LIBRARY_SIZE - first);
        const movies = Array.from({ length: count }, (_, offset) => {
          const id = String(first + offset + 1);
          return { id: { namespace: "tmdb", value: id }, title: `Movie ${id}` };
        });
        const byTitle = request.sort !== MovieSort.ADDED;
        const index = [
          { key: "A", position: 0 },
          { key: "M", position: 450 },
        ];
        return {
          movies,
          page: {
            nextToken: first + count < LIBRARY_SIZE ? String(first + count) : "",
            ...(first === 0 ? { totalSize: LIBRARY_SIZE } : {}),
          },
          index: first === 0 && byTitle ? index : [],
        };
      },
    });
  });
}

test("the rail lists the letters with titles, a dot between those that skip some", async () => {
  openApp("/libraries/1", sixHundredMovies([]));

  const rail = await screen.findByRole("navigation", { name: "Jump to a letter" });

  expect(
    within(rail)
      .getAllByRole("button")
      .map((key) => key.textContent),
  ).toStrictEqual(["A", "M"]);
  expect(rail.querySelectorAll("span[aria-hidden]")).toHaveLength(1);
});

test("a letter past the titles loaded loads pages until its first", async () => {
  const asked: ListMoviesRequest[] = [];
  openApp("/libraries/1", sixHundredMovies(asked));

  fireEvent.click(await screen.findByRole("button", { name: "Movies starting with M" }));

  await waitFor(() => {
    expect(asked.map(({ page }) => page?.token)).toStrictEqual(["", "100", "200", "300", "400"]);
  });
});

test("recently Added has no rail", async () => {
  openApp("/libraries/1?sort=added", sixHundredMovies([]));

  await screen.findByText("600 Movies");

  expect(screen.queryByRole("navigation", { name: /^Jump to/u })).toBeNull();
});
