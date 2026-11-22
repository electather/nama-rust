import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { type BoxSetSchema, BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const alien = { id: { namespace: "tmdb", value: "348" }, title: "Alien" };
const aliens = { id: { namespace: "tmdb", value: "679" }, title: "Aliens" };
const alien3 = { id: { namespace: "tmdb", value: "8077" }, title: "Alien³", year: 1992 };

/** The Alien films: Alien and Aliens in the Movies Library 1, Alien³ missing. */
const alienBoxSet: MessageInitShape<typeof BoxSetSchema> = {
  id: { namespace: "tmdb", value: "8091" },
  name: "Alien Collection",
  overview: "A space horror series.",
  movies: [
    { movieOrShow: { case: "movie", value: { ...alien, libraryId: 1n } } },
    { movieOrShow: { case: "movie", value: { ...aliens, libraryId: 1n } } },
    { movieOrShow: { case: "missingMovie", value: alien3 } },
  ],
};

/**
 * A fake Server with a Movies Library (1) holding Alien, whose Box Set is `boxSet` on its page and
 * by `GetBoxSet`, which finds no other. Alien³ is there to Request.
 */
function server(boxSet?: MessageInitShape<typeof BoxSetSchema>) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({ movie: { ...alien, versions: [{ id: 1n }] }, boxSet }),
      getBoxSet: (request) => {
        if (boxSet === undefined || request.boxSetId?.value !== "8091") {
          throw new ConnectError("no such Box Set", Code.NotFound);
        }
        return { boxSet };
      },
    });
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: () => ({ movie: alien3 }),
      listMovieQualities: () => ({}),
    });
  });
}

/** Each link's name and where it goes, in order. */
function named(links: HTMLElement[]) {
  return links.map((link) => [link.getAttribute("aria-label"), link.getAttribute("href")]);
}

test("a Movie's page shows its Box Set's Movies under its name, the Movie marked", async () => {
  openApp("/libraries/1/movies/tmdb-348", server(alienBoxSet));

  const section = await screen.findByRole("region", { name: "Alien Collection" });
  const [heading, ...posters] = within(section).getAllByRole("link");

  expect(heading?.getAttribute("href")).toBe("/box-sets/tmdb-8091");
  expect(named(posters)).toStrictEqual([
    ["Alien", "/libraries/1/movies/tmdb-348"],
    ["Aliens", "/libraries/1/movies/tmdb-679"],
    ["Alien³", "/movies/tmdb-8077"],
  ]);
  expect(posters.map((poster) => poster.getAttribute("aria-current"))).toStrictEqual([
    "page",
    null,
    null,
  ]);
  expect(within(section).getByText("This Movie")).toBeDefined();
});

test("a Movie with no Box Set shows no row", async () => {
  openApp("/libraries/1/movies/tmdb-348", server());

  await expect(screen.findByRole("heading", { level: 1, name: "Alien" })).resolves.toBeDefined();
  expect(screen.queryByRole("region", { name: "Alien Collection" })).toBeNull();
});

test("a Box Set's page shows its name, overview and Movies in order", async () => {
  openApp("/box-sets/tmdb-8091", server(alienBoxSet));

  await expect(
    screen.findByRole("heading", { level: 1, name: "Alien Collection" }),
  ).resolves.toBeDefined();
  expect(screen.getByText("A space horror series.")).toBeDefined();
  const movies = screen.getByRole("region", { name: "Movies" });
  expect(named(within(movies).getAllByRole("link"))).toStrictEqual([
    ["Alien", "/libraries/1/movies/tmdb-348"],
    ["Aliens", "/libraries/1/movies/tmdb-679"],
    ["Alien³", "/movies/tmdb-8077"],
  ]);
  expect(within(movies).getByText("Not on this Server")).toBeDefined();
});

test("a Box Set's missing Movie opens its page to Request", async () => {
  openApp("/box-sets/tmdb-8091", server(alienBoxSet));

  fireEvent.click(await screen.findByRole("link", { name: "Alien³" }));

  await expect(screen.findByRole("heading", { level: 1, name: "Alien³" })).resolves.toBeDefined();
  expect(globalThis.location.pathname).toBe("/movies/tmdb-8077");
});

test("a Box Set the Server doesn't show you isn't found", async () => {
  openApp("/box-sets/tmdb-1", server(alienBoxSet));

  await expect(
    screen.findByRole("heading", { level: 1, name: "Page not found" }),
  ).resolves.toBeDefined();
});
