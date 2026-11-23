import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import {
  MatchService,
  TitleKind,
  type UnmatchedItemSchema,
  UnmatchedReason,
} from "@/gen/nama/v1/match_pb";
import { admin, eventFeed, fakeServer } from "@/test-app";
import { openSettings } from "@/test-settings";

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
  openSettings(
    "needs-a-match",
    adminServer(events.stream, (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => ({ items }),
        searchTitles: () => ({
          candidates: [{ id: { namespace: "tmdb", value: "348" }, title: "Alien", year: 1979 }],
        }),
        setMatch: ({ itemId }) => {
          items = [];
          events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId } } });
          return { matched: 1n };
        },
      });
    }),
  );

  await expect(screen.findByText(alien.path)).resolves.toBeDefined();
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
  [
    "an Episode its Show doesn’t have says so",
    { reason: UnmatchedReason.NOT_IN_SHOW },
    "Its Show has no such Episode. Matching it to another Show may help.",
  ],
  [
    "an Item whose Match was cleared says so",
    { reason: UnmatchedReason.CLEARED_BY_HAND },
    "Unmatched by hand.",
  ],
  [
    "an Item waiting for a source that searches says so",
    { reason: UnmatchedReason.NO_SEARCHING_SOURCE },
    "None of its Library’s title sources can search for titles. Once one can, the next scan tries again.",
  ],
])("%s on Needs a Match", async (_, why, shown) => {
  const item = { itemId: 7n, path: "/srv/shows/The Office/S01E01.mkv", kind: TitleKind.SHOW };
  openSettings(
    "needs-a-match",
    adminServer(new ReadableStream(), (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => ({ items: [{ ...item, title: "The Office", ...why }] }),
      });
    }),
  );

  await expect(screen.findByText(shown)).resolves.toBeDefined();
});

test("needs a Match names an Item parsed as Season 0 a Special", async () => {
  const item = {
    itemId: 7n,
    path: "/srv/shows/Severance/S00E02.mkv",
    kind: TitleKind.SHOW,
    title: "Severance",
    season: 0,
    episode: 2,
  };
  openSettings(
    "needs-a-match",
    adminServer(new ReadableStream(), (router) => {
      router.service(MatchService, { listUnmatchedItems: () => ({ items: [item] }) });
    }),
  );

  await expect(
    screen.findByRole("button", { name: "Find a Match for Severance · Specials, Episode 2" }),
  ).resolves.toBeDefined();
});

test("needs a Match shows why once Matching finishes", async () => {
  const events = eventFeed();
  const item = { itemId: 7n, path: "/srv/films/Alien.1979.mkv", kind: TitleKind.MOVIE };
  let reason: UnmatchedReason = UnmatchedReason.UNSPECIFIED;
  let listed = 0;
  openSettings(
    "needs-a-match",
    adminServer(events.stream, (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => {
          listed += 1;
          return { items: [{ ...item, title: "Alien", reason }] };
        },
      });
    }),
  );
  await expect(screen.findByText(item.path)).resolves.toBeDefined();
  // The scan finished before Matching did. Its Event opens the stream, and the catch-up refetch
  // that follows has to land first, or it would be the one to show the reason.
  events.send({ event: { case: "scanFinished", value: { libraryId: 1n } } });
  await waitFor(() => {
    expect(listed).toBe(3);
  });

  reason = UnmatchedReason.NO_TITLE_FITS;
  events.send({ event: { case: "matchingFinished", value: { libraryId: 1n, unmatched: 1n } } });

  await expect(
    screen.findByText("No title fits its name. Renaming the file or adding an ID Hint may help."),
  ).resolves.toBeDefined();
});

test("an Item Matched elsewhere leaves Needs a Match when its Event arrives", async () => {
  const events = eventFeed();
  const items = [
    { itemId: 7n, path: "/srv/films/Alien.1979.mkv", kind: TitleKind.MOVIE, title: "Alien" },
  ];
  let unmatched = (): object => ({ items });
  openSettings(
    "needs-a-match",
    adminServer(events.stream, (router) => {
      router.service(MatchService, {
        listUnmatchedItems: () => unmatched(),
      });
    }),
  );
  await screen.findByText("/srv/films/Alien.1979.mkv");

  // Matched automatically, or by another Admin.
  // From now on a refetch never answers, so only the Event can take the Item off.
  unmatched = async () => {
    await new Promise(() => {});
    return {};
  };
  events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId: 7n } } });

  await expect(screen.findByText("Everything is Matched")).resolves.toBeDefined();
});

test("needs a Match says when everything is Matched", async () => {
  openSettings(
    "needs-a-match",
    adminServer(new ReadableStream(), (router) => {
      router.service(MatchService, { listUnmatchedItems: () => ({}) });
    }),
  );

  await expect(screen.findByText("Everything is Matched")).resolves.toBeDefined();
});
