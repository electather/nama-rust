import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { type MovieHitSchema, RequestService } from "@/gen/nama/v1/request_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

type Similar = MessageInitShape<typeof MovieHitSchema>[];

/** Heat, a Similar title in the Movies Library 1. */
const heat = { id: { namespace: "tmdb", value: "949" }, title: "Heat", year: 1995, libraryId: 1n };

/**
 * A fake Server with a Movies Library (1) holding Alien, whose Similar titles in a Library are
 * `similar` on its page and its request page alike.
 */
function server(similar: Similar) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const alien = { id: { namespace: "tmdb", value: "348" }, title: "Alien" };
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({ movie: { ...alien, versions: [{ id: 1n }] }, similar }),
    });
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: () => ({ movie: { ...alien, similar } }),
      listMovieQualities: () => ({}),
    });
  });
}

/** The Similar titles row under `heading`, once the page shows it. */
async function row(heading: string) {
  const section = await screen.findByRole("region", { name: heading });
  return within(section).getAllByRole("link");
}

test("a Movie's page shows its Similar titles under Similar titles, each opening in its Library", async () => {
  openApp("/libraries/1/movies/tmdb-348", server([heat]));

  const links = await row("Similar titles");

  expect(links.map((link) => [link.textContent, link.getAttribute("href")])).toStrictEqual([
    ["Heat1995", "/libraries/1/movies/tmdb-949"],
  ]);
});

test("a Movie to Request shows its Similar titles under On this Server", async () => {
  openApp("/movies/tmdb-348", server([heat]));

  const links = await row("On this Server");

  expect(links.map((link) => link.getAttribute("href"))).toStrictEqual([
    "/libraries/1/movies/tmdb-949",
  ]);
});

test("a Movie with no Similar title in a Library shows no row", async () => {
  openApp("/libraries/1/movies/tmdb-348", server([]));

  await expect(screen.findByRole("heading", { level: 1, name: "Alien" })).resolves.toBeDefined();
  expect(screen.queryByRole("region", { name: "Similar titles" })).toBeNull();
});
