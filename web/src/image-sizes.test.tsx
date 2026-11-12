import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const poster = "/media/artwork/tmdb/%2Fposter.jpg";
const backdrop = "/media/artwork/tmdb/%2Fbackdrop.jpg";
const alien = { id: { case: "tmdb" as const, value: 348n } };

/** A fake Server that resizes to 320 and 640 pixels wide, with one Movie in Library 1. */
function serverWithAMovie() {
  return fakeServer(
    {
      signedIn: newUser,
      libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }],
      serverService: { getServerInfo: () => ({ name: "Living Room", imageWidths: [320, 640] }) },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        listMovies: () => ({ movies: [{ id: alien, title: "Alien", posterUrl: poster }] }),
        getMovie: () => ({
          movie: { title: "Alien", backdropUrl: backdrop, versions: [{ id: 1n }] },
        }),
      });
    },
  );
}

/** The `srcset` of the one image whose `src` is `url`. */
function srcSetOf(url: string) {
  return document.querySelector(`img[src="${url}"]`)?.getAttribute("srcset");
}

test("a Poster offers a copy at each width the Server resizes to", async () => {
  openApp("/libraries/1", serverWithAMovie());

  await screen.findByText("Alien");

  expect(srcSetOf(poster)).toBe(`${poster}?width=320 320w, ${poster}?width=640 640w`);
});

test("a Backdrop offers a copy at each width the Server resizes to", async () => {
  openApp("/libraries/1/movies/tmdb-348", serverWithAMovie());

  await screen.findByText("Alien");

  expect(srcSetOf(backdrop)).toBe(`${backdrop}?width=320 320w, ${backdrop}?width=640 640w`);
});
