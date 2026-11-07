import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  EventSchema,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { MatchService, TitleKind, type UnmatchedItemSchema } from "@/gen/nama/v1/match_pb";
import { Role } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, fill, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

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

test("an Item Matched elsewhere leaves Needs a Match when its Event arrives", async () => {
  const events = eventFeed();
  const items = [
    { itemId: 7n, path: "/srv/films/Alien.1979.mkv", kind: TitleKind.MOVIE, title: "Alien" },
  ];
  let unmatched = (): object => ({ items });
  openApp(
    "/admin/needs-a-match",
    adminServer(events.stream, (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => unmatched(),
      });
    }),
  );
  await screen.findByText("/srv/films/Alien.1979.mkv");

  // Matched automatically, or by another Admin.
  // From now on a refetch never answers, so only the Event can take the Item off.
  unmatched = () => new Promise<never>(() => {});
  events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId: 7n } } });

  expect(await screen.findByText("Everything is Matched.")).toBeDefined();
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

test("Needs a Match says when the picked Library isn't on the Server", async () => {
  openApp(
    "/admin/needs-a-match?library=9",
    adminServer(new ReadableStream(), (router) => {
      router.service(MatchService, { listUnmatchedItems: () => ({}) });
    }),
  );

  expect(await screen.findByText("That Library isn’t on the Server.")).toBeDefined();
  expect(screen.queryByText("Everything is Matched.")).toBeNull();
});

test("a Library that isn't on the Server shows the not-found page, under the app chrome", async () => {
  openApp(
    "/admin/libraries/9",
    adminServer(new ReadableStream(), (router) => {
      router.service(LibraryService, {
        getLibrary: () => {
          throw new ConnectError("there is no Library 9", Code.NotFound);
        },
      });
    }),
  );

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
  expect(screen.getByRole("navigation", { name: "Admin" })).toBeDefined();
});

/** A fake Server whose Library changes are refused for the request field `field` with `message`. */
function serverRefusingLibraryField(field: string, message: string) {
  const refuse = () => {
    throw refused(Code.InvalidArgument, message, { field });
  };
  return adminServer(new ReadableStream(), (router) => {
    router.service(LibraryService, {
      listLibraries: () => ({ libraries: [films] }),
      getLibrary: () => ({ library: films }),
      createLibrary: refuse,
      updateLibrary: refuse,
    });
  });
}

describe.each([
  ["/admin/libraries", "Create Library"],
  ["/admin/libraries/1", "Save changes"],
])("on %s", (path, action) => {
  test.each([
    ["name", "Name", "a Library's name can't be blank", "A Library's name can't be blank."],
    [
      "folders[2]",
      "Folders",
      "the folder srv is not an absolute path",
      "The folder srv is not an absolute path.",
    ],
  ])("a refused %s shows under its field", async (field, label, message, shown) => {
    openApp(path, serverRefusingLibraryField(field, message));
    await screen.findByRole("button", { name: action });
    fill("Name", "Films");
    fireEvent.change(screen.getByRole("textbox", { name: "Folders" }), {
      target: { value: "/srv/films" },
    });
    fireEvent.click(screen.getByRole("button", { name: action }));

    await expectUnderField(label, shown);
  });
});
