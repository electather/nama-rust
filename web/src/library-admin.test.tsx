import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError, type ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import {
  MatchService,
  TitleKind,
  type UnmatchedItemSchema,
  UnmatchedReason,
} from "@/gen/nama/v1/match_pb";

import { admin, eventFeed, fakeServer, fill, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

/** A fake Server with the Library `films`, whose Admin is signed in, with `more` services. */
function adminServer(
  events: ReadableStream<SubscribeResponse>,
  more: (router: ConnectRouter) => void,
) {
  return fakeServer({ signedIn: admin, events }, {}, {}, (router) => {
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

test.each([
  [
    "an Ambiguous Item names the titles that tied",
    {
      reason: UnmatchedReason.AMBIGUOUS,
      tiedTitles: [
        { title: "The Office", year: 2005 },
        { title: "The Office", year: 2001 },
      ],
    },
    "More than one title fits: The Office (2005) or The Office (2001).",
  ],
  [
    "an Item no title fits says so",
    { reason: UnmatchedReason.NO_TITLE_FITS },
    "No title fits its name. Renaming the file or adding an ID Hint may help.",
  ],
  [
    "an Item whose ID Hint names no title says so",
    { reason: UnmatchedReason.UNKNOWN_ID_HINT },
    "Its ID Hint names no title.",
  ],
])("%s on Needs a Match", async (_, why, shown) => {
  const item = { itemId: 7n, path: "/srv/shows/The Office/S01E01.mkv", kind: TitleKind.SHOW };
  openApp(
    "/admin/needs-a-match",
    adminServer(new ReadableStream(), (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => ({ items: [{ ...item, title: "The Office", ...why }] }),
      });
    }),
  );

  expect(await screen.findByText(shown)).toBeDefined();
});

test("Needs a Match shows why once Matching finishes", async () => {
  const events = eventFeed();
  const item = { itemId: 7n, path: "/srv/films/Alien.1979.mkv", kind: TitleKind.MOVIE };
  let reason: UnmatchedReason = UnmatchedReason.UNSPECIFIED;
  let listed = 0;
  openApp(
    "/admin/needs-a-match",
    adminServer(events.stream, (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => {
          listed += 1;
          return { items: [{ ...item, title: "Alien", reason }] };
        },
      });
    }),
  );
  expect(await screen.findByText(item.path)).toBeDefined();
  // The scan finished before Matching did. Its Event opens the stream, and the catch-up refetch
  // that follows has to land first, or it would be the one to show the reason.
  events.send({ event: { case: "scanFinished", value: { libraryId: 1n } } });
  await waitFor(() => {
    expect(listed).toBe(3);
  });

  reason = UnmatchedReason.NO_TITLE_FITS;
  events.send({ event: { case: "matchingFinished", value: { libraryId: 1n, unmatched: 1n } } });

  expect(
    await screen.findByText(
      "No title fits its name. Renaming the file or adding an ID Hint may help.",
    ),
  ).toBeDefined();
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
    ["Name", "name", "a Library's name can't be blank", "A Library's name can't be blank."],
    [
      "Folders",
      "folders[2]",
      "the folder srv is not an absolute path",
      "The folder srv is not an absolute path.",
    ],
  ])("a refused %s shows under its field", async (label, field, message, shown) => {
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
