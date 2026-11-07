import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { Role } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";

afterEach(cleanup);

/** A Server whose Library 1 has The Matrix in two Versions, the larger being Version 8. */
const server = fakeServer({ signedIn: Role.MEMBER }, {}, {}, (router) => {
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
});

test("a file the browser can't play shows a message instead of the player", async () => {
  openApp("/libraries/1/movies/tmdb-603/play?version=7", server);

  fireEvent.error(await findVideo());

  expect(await screen.findByText(/This browser can’t play this file/u)).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});
