import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type MovieSchema } from "@/gen/nama/v1/browse_pb";
import {
  EventService,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { Role } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";

afterEach(cleanup);

/** A fake Server with a Movies Library (1) and a Shows Library (2), browsed through `browse`. */
function serverToBrowse(
  browse: Partial<ServiceImpl<typeof BrowseService>>,
  events?: AsyncIterable<SubscribeResponse>,
) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  return fakeServer({ signedIn: Role.MEMBER, libraries }, {}, {}, (router) => {
    router.service(BrowseService, browse);
    if (events) router.service(EventService, { subscribe: () => events });
  });
}

test("an empty Library says so", async () => {
  openApp("/libraries/1", serverToBrowse({ listMovies: () => ({ movies: [] }) }));

  expect(await screen.findByRole("heading", { name: "Films" })).toBeDefined();
  expect(screen.getByText("Nothing in this Library yet")).toBeDefined();
});

test("an unknown Movie shows the not-found page, under the app chrome", async () => {
  const transport = serverToBrowse({
    getMovie: () => {
      throw new ConnectError("no such Movie", Code.NotFound);
    },
  });
  openApp("/libraries/1/movies/tmdb-404", transport);

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
  expect(screen.getByRole("link", { name: "Libraries" })).toBeDefined();
});

test("a Season the Show doesn't have falls back to its first, after the Specials", async () => {
  const transport = serverToBrowse({
    getShow: () => ({
      show: { title: "Severance" },
      seasons: [{ number: 0 }, { number: 1 }, { number: 2 }],
    }),
    listEpisodes: ({ seasonNumber }) => ({
      episodes: [{ seasonNumber, episodeNumber: 1, title: `Opener of Season ${seasonNumber}` }],
    }),
  });
  openApp("/libraries/2/shows/tmdb-95396?season=9", transport);

  expect(await screen.findByText(/Opener of Season 1/u)).toBeDefined();
  expect(screen.queryByText(/Opener of Season (0|9)/u)).toBeNull();
});

test("a Library's page shows a Movie as soon as it's Matched", async () => {
  const listed: MessageInitShape<typeof MovieSchema>[] = [];
  let match: (() => void) | undefined;
  const matched = new Promise<void>((resolve) => {
    match = resolve;
  });
  async function* events() {
    await matched;
    yield create(SubscribeResponseSchema, {
      message: {
        case: "event",
        value: { event: { case: "itemMatched", value: { libraryId: 1n } } },
      },
    });
  }
  openApp("/libraries/1", serverToBrowse({ listMovies: () => ({ movies: listed }) }, events()));
  await screen.findByText("Nothing in this Library yet");

  listed.push({ id: { id: { case: "tmdb", value: 348n } }, title: "Alien" });
  match?.();

  expect(await screen.findByText("Alien")).toBeDefined();
});
