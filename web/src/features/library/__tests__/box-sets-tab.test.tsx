import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

/** A fake Server with a Movies Library (1) holding Alien, of the Alien films, and a Shows one (2). */
function server() {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  const alien = { id: { namespace: "tmdb", value: "348" }, title: "Alien" };
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listUnmatched: () => ({}),
      listGenres: () => ({}),
      listMovies: () => ({ movies: [alien], page: { totalSize: 1 } }),
      listShows: () => ({ page: { totalSize: 0 } }),
      listBoxSets: () => ({
        boxSets: [{ id: { namespace: "tmdb", value: "8091" }, name: "Alien Collection" }],
        page: { totalSize: 1 },
      }),
    });
  });
}

test("a Movies Library's Box Sets tab lists its Box Sets, each opening its page", async () => {
  openApp("/libraries/1", server());

  fireEvent.click(await screen.findByRole("tab", { name: "Box Sets" }));

  const boxSet = await screen.findByRole("link", { name: "Alien Collection" });
  expect(boxSet.getAttribute("href")).toBe("/box-sets/tmdb-8091");
  expect(screen.getByText("1 Box Set")).toBeDefined();
  expect(globalThis.location.search).toBe("?view=box-sets");
});

test("a Movies Library opens on its Movies tab", async () => {
  openApp("/libraries/1", server());

  const movies = await screen.findByRole("tab", { name: "Movies" });

  expect(movies.getAttribute("aria-selected")).toBe("true");
  expect(screen.getByText("1 Movie")).toBeDefined();
});

test("a Shows Library has no Box Sets tab", async () => {
  openApp("/libraries/2", server());

  await expect(screen.findByRole("heading", { level: 1, name: "TV" })).resolves.toBeDefined();
  expect(screen.queryByRole("tab", { name: "Box Sets" })).toBeNull();
});

test("going to a Library's Box Sets and back keeps its Movies' sort", async () => {
  openApp("/libraries/1?sort=released", server());

  fireEvent.click(await screen.findByRole("tab", { name: "Box Sets" }));
  await screen.findByRole("link", { name: "Alien Collection" });
  fireEvent.click(screen.getByRole("tab", { name: "Movies" }));

  await screen.findByText("1 Movie");
  expect(globalThis.location.search).toBe("?sort=released");
});
