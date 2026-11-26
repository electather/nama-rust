import { cleanup, screen } from "@testing-library/react";
import { rgbaToThumbHash } from "thumbhash";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const poster = "/media/artwork/tmdb/%2Fposter.jpg";
const backdrop = "/media/artwork/tmdb/%2Fbackdrop.jpg";
const heatPoster = "/media/artwork/tmdb/%2Fheat.jpg";
const logo = "/media/artwork/tmdb/%2Flogo.png";
const alien = { namespace: "tmdb", value: "348" };
const heat = { namespace: "tmdb", value: "949" };
/** A grey 2 by 3 image's thumbhash. */
const thumbhash = rgbaToThumbHash(2, 3, new Uint8Array(2 * 3 * 4).fill(128));

/** Widths a fake Server resizes to, and those it makes Posters, Backdrops and Title Logos at. */
const widths = {
  imageWidths: [320, 480, 640, 1280],
  posterWidths: [320, 480],
  backdropWidths: [640, 1280],
  titleLogoWidths: [480],
};

/**
 * A fake Server that lists `offered` widths, with Library 1, shown by `backdrop`, and two
 * Movies in it: Alien, whose poster has a thumbhash, and Heat, whose poster has none and which is
 * Alien's Similar title.
 */
function serverWithAMovie(offered: Partial<typeof widths> = widths) {
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
      serverService: { getServerInfo: () => ({ name: "Living Room", ...offered }) },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        listUnmatched: () => ({}),
        listMovies: () => ({
          movies: [
            { id: alien, title: "Alien", posterUrl: poster, posterThumbhash: thumbhash },
            { id: heat, title: "Heat", posterUrl: heatPoster },
          ],
        }),
        listContinueWatching: () => ({}),
        listGenres: () => ({}),
        getMovie: () => ({
          movie: {
            title: "Alien",
            backdropUrl: backdrop,
            titleLogoUrl: logo,
            versions: [{ id: 1n }],
            trailers: [{ name: "Teaser", url: "https://vimeo.com/123456789" }],
          },
          similar: [{ id: heat, title: "Heat", libraryId: 1n, posterUrl: heatPoster }],
        }),
      });
    },
  );
}

/** The `srcset` of the one image whose `src` is `url`. */
function srcSetOf(url: string) {
  return document.querySelector(`img[src="${url}"]`)?.getAttribute("srcset");
}

/** The `srcset` of each image whose `src` is `url`. */
function srcSetsOf(url: string) {
  return [...document.querySelectorAll(`img[src="${url}"]`)].map((img) =>
    img.getAttribute("srcset"),
  );
}

/** The thumbhash preview drawn in the box of the one image whose `src` is `url`, if there is one. */
function previewBehind(url: string) {
  const box = document.querySelector(`img[src="${url}"]`)?.parentElement;
  return box?.querySelector('img[src^="data:image/png"]');
}

test("a Poster offers a copy at only each width the Server makes Posters at ahead", async () => {
  openApp("/libraries/1", serverWithAMovie());

  await screen.findByText("Alien");

  expect(srcSetOf(poster)).toBe(`${poster}?width=320 320w, ${poster}?width=480 480w`);
});

test("a shelf's Poster offers a copy at only each width the Server makes Posters at ahead", async () => {
  openApp("/libraries/1/movies/tmdb-348", serverWithAMovie());

  await screen.findByRole("link", { name: "Heat" });

  expect(srcSetOf(heatPoster)).toBe(`${heatPoster}?width=320 320w, ${heatPoster}?width=480 480w`);
});

test("a Backdrop and a Title Logo offer a copy at only each width the Server makes them at ahead", async () => {
  openApp("/libraries/1/movies/tmdb-348", serverWithAMovie());

  await screen.findByRole("img", { name: "Alien" });
  await screen.findByRole("region", { name: "Trailers" });

  // The page's Backdrop, and the Teaser's tile showing it.
  const backdropSrcSet = `${backdrop}?width=640 640w, ${backdrop}?width=1280 1280w`;
  expect(srcSetsOf(backdrop)).toStrictEqual([backdropSrcSet, backdropSrcSet]);
  expect(srcSetOf(logo)).toBe(`${logo}?width=480 480w`);
});

test("a Poster from a Server that makes none ahead offers each width it resizes to", async () => {
  openApp("/libraries/1", serverWithAMovie({ imageWidths: [320, 640] }));

  await screen.findByText("Alien");

  expect(srcSetOf(poster)).toBe(`${poster}?width=320 320w, ${poster}?width=640 640w`);
});

test("a Poster shows its thumbhash in its box until the image covers it", async () => {
  openApp("/libraries/1", serverWithAMovie());

  await screen.findByText("Alien");

  const preview = previewBehind(poster);
  expect(preview).not.toBeNull();
  // Drawn before the image, so the image covers it once it loads.
  expect(preview?.nextElementSibling?.getAttribute("src")).toBe(poster);
});

test("a Poster without a thumbhash shows only its image", async () => {
  openApp("/libraries/1", serverWithAMovie());

  await screen.findByText("Heat");

  expect(previewBehind(heatPoster)).toBeNull();
});
