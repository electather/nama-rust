import { type MessageInitShape, create } from "@bufbuild/protobuf";
import type { ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  EventSchema,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { MatchService, TitleKind, type UnmatchedItemSchema } from "@/gen/nama/v1/match_pb";
import { Role } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, openApp } from "./test-app";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

/** Events for a test to send the app. What's sent before the app subscribes waits for it. */
function eventFeed() {
  let feed: ReadableStreamDefaultController<SubscribeResponse> | undefined;
  return {
    stream: new ReadableStream<SubscribeResponse>({
      start: (controller) => {
        feed = controller;
      },
    }),
    send: (event: MessageInitShape<typeof EventSchema>) => {
      const value = create(EventSchema, event);
      feed?.enqueue(create(SubscribeResponseSchema, { message: { case: "event", value } }));
    },
  };
}

/** A fake Server with the Library `films`, whose Admin is signed in, with `more` services. */
function adminServer(
  events: ReadableStream<SubscribeResponse>,
  more: (router: ConnectRouter) => void,
) {
  return fakeServer({ signedIn: Role.ADMIN, events }, {}, {}, (router) => {
    router.service(LibraryService, { listLibraries: () => ({ libraries: [films] }) });
    more(router);
  });
}

test("a scan Event updates a Library's progress without a reload", async () => {
  const events = eventFeed();
  openApp(
    "/admin/libraries",
    adminServer(events.stream, () => {}),
  );
  await screen.findByRole("link", { name: "Films" });

  events.send({
    event: { case: "scanProgress", value: { libraryId: 1n, filesDone: 3n, filesTotal: 10n } },
  });
  expect(await screen.findByText("Scanning: 3 of 10 files")).toBeDefined();

  events.send({
    event: { case: "scanFinished", value: { libraryId: 1n, added: 9n, updated: 0n, removed: 1n } },
  });
  expect(await screen.findByText("Scanned: 9 new, 0 changed and 1 gone")).toBeDefined();
});

test("setting a Match takes the Item off Needs a Match", async () => {
  const events = eventFeed();
  const alien = {
    itemId: 7n,
    path: "/srv/films/Alien.1979.mkv",
    kind: TitleKind.MOVIE,
    title: "Alien",
    year: 1979,
  };
  let items: MessageInitShape<typeof UnmatchedItemSchema>[] = [alien];
  openApp(
    "/admin/needs-a-match",
    adminServer(events.stream, (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => ({ items }),
        searchMetadata: () => ({
          candidates: [{ id: { id: { case: "tmdb", value: 348n } }, title: "Alien", year: 1979 }],
        }),
        setMatch: ({ itemId }) => {
          items = [];
          events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId } } });
          return { matched: 1n };
        },
      });
    }),
  );

  expect(await screen.findByText(alien.path)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Find a Match for Alien (1979)" }));
  const results = await screen.findByRole("list", { name: "Titles found" });
  fireEvent.click(within(results).getByRole("button", { name: "Match to Alien (1979)" }));

  await waitFor(() => {
    expect(screen.queryByText(alien.path)).toBeNull();
  });
});

test("Needs a Match says when everything is Matched", async () => {
  openApp(
    "/admin/needs-a-match",
    adminServer(new ReadableStream(), (router) => {
      router.service(MatchService, { listUnmatchedItems: () => ({}) });
    }),
  );

  expect(await screen.findByText("Everything is Matched.")).toBeDefined();
});
