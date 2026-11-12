import { cleanup, screen } from "@testing-library/react";
import { rgbaToThumbHash } from "thumbhash";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const poster = "/media/artwork/tmdb/%2Fposter.jpg";
const backdrop = "/media/artwork/tmdb/%2Fbackdrop.jpg";
const heatPoster = "/media/artwork/tmdb/%2Fheat.jpg";
const alien = { id: { case: "tmdb" as const, value: 348n } };
const heat = { id: { case: "tmdb" as const, value: 949n } };
/** A grey 2 by 3 image's thumbhash. */
const thumbhash = rgbaToThumbHash(2, 3, new Uint8Array(2 * 3 * 4).fill(128));

/**
 * A fake Server that resizes to 320 and 640 pixels wide, with Library 1, shown by `backdrop`, and
 * two Movies in it: Alien, whose poster has a thumbhash, and Heat, whose poster has none.
 */
function serverWithAMovie() {
  return fakeServer(
    {
      signedIn: newUser,
      libraries: [
        {
          id: 1n,
          name: "Films",
          kind: LibraryKind.MOVIES,
          backdropUrl: backdrop,
          backdropThumbhash: thumbhash,
        },
      ],
      serverService: { getServerInfo: () => ({ name: "Living Room", imageWidths: [320, 640] }) },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        listMovies: () => ({
          movies: [
            { id: alien, title: "Alien", posterUrl: poster, posterThumbhash: thumbhash },
            { id: heat, title: "Heat", posterUrl: heatPoster },
          ],
        }),
        listContinueWatching: () => ({}),
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

/** The thumbhash preview drawn in the box of the one image whose `src` is `url`, if there is one. */
function previewBehind(url: string) {
  const box = document.querySelector(`img[src="${url}"]`)?.parentElement;
  return box?.querySelector('img[src^="data:image/png"]');
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

test("a Poster shows its thumbhash in its box until the image covers it", async () => {
  openApp("/libraries/1", serverWithAMovie());

  await screen.findByText("Alien");

  const preview = previewBehind(poster);
  expect(preview).toBeTruthy();
  // Drawn before the image, so the image covers it once it loads.
  expect(preview?.nextElementSibling?.getAttribute("src")).toBe(poster);
});

test("a Poster without a thumbhash shows only its image", async () => {
  openApp("/libraries/1", serverWithAMovie());

  await screen.findByText("Heat");

  expect(previewBehind(heatPoster)).toBeNull();
});

test("a Library's Backdrop offers a copy at each width and shows its thumbhash until it loads", async () => {
  openApp("/", serverWithAMovie());

  await screen.findByText("Films");

  expect(srcSetOf(backdrop)).toBe(`${backdrop}?width=320 320w, ${backdrop}?width=640 640w`);
  expect(previewBehind(backdrop)?.nextElementSibling?.getAttribute("src")).toBe(backdrop);
});
