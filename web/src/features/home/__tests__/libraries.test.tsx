import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { continueWatchingRow } from "@/test-home-rows";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const NEWEST = "/media/artwork/tmdb/%2Falien-backdrop.jpg";

test("home shows each Library by its Backdrop and kind, opening it, and one without by its name", async () => {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES, backdropUrl: NEWEST },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listFeatured: () => ({}),
      listContinueWatching: () => ({}),
      listHomeRows: () => ({ rows: [continueWatchingRow] }),
    });
  });
  openApp("/", transport);

  const section = await screen.findByRole("region", { name: "Libraries" });
  const films = within(section).getByRole("link", { name: /Films/u });
  const tv = within(section).getByRole("link", { name: /TV/u });

  expect(films.getAttribute("href")).toBe("/libraries/1");
  expect(films.querySelector("img")?.getAttribute("src")).toBe(NEWEST);
  expect(within(films).getByText("Movies")).toBeDefined();
  expect(tv.getAttribute("href")).toBe("/libraries/2");
  expect(tv.querySelector("img")).toBeNull();
  expect(within(tv).getByText("Shows")).toBeDefined();
});

const alien = {
  libraryId: 1n,
  movieOrEpisode: {
    case: "movie",
    value: { id: { namespace: "tmdb", value: "348" }, title: "Alien" },
  },
} as const;

test("home still shows its other rows when the Libraries fail to load", async () => {
  const transport = fakeServer(
    {
      signedIn: newUser,
      libraryService: {
        listLibraries: () => {
          throw new ConnectError("broken", Code.Internal);
        },
      },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        listFeatured: () => ({}),
        listContinueWatching: () => ({ entries: [alien] }),
        listHomeRows: () => ({ rows: [continueWatchingRow] }),
      });
    },
  );
  openApp("/", transport);

  await screen.findByRole("link", { name: /Alien/u });
  expect(screen.queryByRole("region", { name: "Libraries" })).toBeNull();
});
