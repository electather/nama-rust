import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** A Server whose Library 1 has The Matrix in three Versions, the largest being Version 8. */
const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
const server = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
  router.service(BrowseService, {
    getMovie: () => ({
      movie: {
        id: { id: { case: "tmdb", value: 603n } },
        title: "The Matrix",
        versions: [
          { id: 7n, fileName: "The Matrix (1999) 720p.mkv", size: 2_000_000_000n },
          { id: 8n, fileName: "The Matrix (1999) 2160p.mkv", size: 40_000_000_000n },
          { id: 9n, fileName: "The Matrix (1999) 1080p.mkv", size: 9_000_000_000n },
        ],
      },
    }),
  });
});

function findVideo() {
  return screen.findByLabelText("The Matrix");
}

test("without a version, the player plays the largest", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);

  expect((await findVideo()).getAttribute("src")).toBe("/media/versions/8");
  expect(screen.queryByRole("navigation", { name: "Main" })).toBeNull();
});

test("a file the browser can't play shows a message instead of the player", async () => {
  openApp("/libraries/1/movies/tmdb-603/play?version=7", server);

  fireEvent.error(await findVideo());

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});

test("an address with no title to play keeps the app chrome, so there's a way out", async () => {
  openApp("/libraries/2/movies/tmdb-603/play", server);

  expect(await screen.findByText("Page not found")).toBeDefined();
  expect(screen.getByRole("navigation", { name: "Main" })).toBeDefined();
});
