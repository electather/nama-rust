import { Code } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { MatchService, TitleKind } from "@/gen/nama/v1/match_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { type ServerState, admin, eventFeed, fakeServer, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];

const ALIEN = { itemId: 3n, versionId: 7n, kind: TitleKind.MOVIE, title: "Alien", year: 1979 };

/**
 * A fake Server, signed in with `signedIn`, whose Library 1 lists Alien as Unmatched and plays its
 * Version 7, until `matches` or `SetMatch` Matches it: from then on it refuses Alien's Version as
 * Unmatched, counting each refusal in `refusals`, and `tellsTheApp` sends the Event that says so.
 * Each Version it plans goes in `planned`, and each Item `SetMatch` gets in `setMatches`.
 */
function alienUnmatched(planned: bigint[], signedIn: ServerState["signedIn"] = newUser) {
  const events = eventFeed();
  let matched = false;
  let refusals = 0;
  const setMatches: bigint[] = [];
  const transport = fakeServer({ signedIn, libraries, events: events.stream }, {}, {}, (router) => {
    router.service(BrowseService, {
      listMovies: () => ({}),
      listGenres: () => ({}),
      listUnmatched: () => ({ items: matched ? [] : [ALIEN] }),
      getUnmatchedVersion: () => {
        if (matched) {
          refusals += 1;
          throw refused(Code.NotFound, "there is no such Version", {
            reason: RefusalReason.VERSION_NOT_FOUND,
          });
        }
        return { item: ALIEN, version: { id: 7n, fileName: "Alien (1979).mkv", size: 1n } };
      },
    });
    router.service(PlaybackService, {
      planPlayback: (request) => {
        planned.push(request.versionId);
        return { plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" } };
      },
    });
    router.service(MatchService, {
      searchTitles: () => ({
        candidates: [{ id: { namespace: "tmdb", value: "348" }, title: "Alien", year: 1979 }],
      }),
      setMatch: (request) => {
        setMatches.push(request.itemId);
        matched = true;
        return { matched: 1n };
      },
    });
  });
  return {
    transport,
    matches: () => {
      matched = true;
    },
    tellsTheApp: () => {
      events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId: 1n } } });
    },
    refusals: () => refusals,
    setMatches,
  };
}

test("play on a Not matched Item plays its Version, titled by what its name says", async () => {
  const planned: bigint[] = [];
  openApp("/libraries/1", alienUnmatched(planned).transport);

  fireEvent.click(await screen.findByRole("link", { name: "Play Alien (1979)" }));

  await expect(screen.findByLabelText("Alien (1979)")).resolves.toBeDefined();
  expect(planned).toEqual([7n]);
});

test("an Unmatched Item Matched while it plays keeps playing", async () => {
  const server = alienUnmatched([]);
  openApp("/libraries/1/unmatched/7/play", server.transport);
  await screen.findByLabelText("Alien (1979)");

  server.matches();
  server.tellsTheApp();
  await waitFor(() => {
    expect(server.refusals()).toBeGreaterThan(0);
  });
  // The refusal reaches the page a few ticks after the Server sends it.
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 20);
    });
  });

  expect(screen.getByLabelText("Alien (1979)")).toBeDefined();
});

test("an Unmatched Item says its progress isn’t saved, without Match it for a member", async () => {
  openApp("/libraries/1/unmatched/7/play", alienUnmatched([]).transport);

  await expect(screen.findByText("Not matched, so progress isn’t saved.")).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Match it" })).toBeNull();
});

test("the note goes once the Item is Matched elsewhere while it plays", async () => {
  const server = alienUnmatched([]);
  openApp("/libraries/1/unmatched/7/play", server.transport);
  await screen.findByText("Not matched, so progress isn’t saved.");

  server.matches();
  server.tellsTheApp();

  await waitFor(() => {
    expect(screen.queryByText("Not matched, so progress isn’t saved.")).toBeNull();
  });
  expect(screen.getByLabelText("Alien (1979)")).toBeDefined();
});

test("match it, for an Admin, opens the Item's Match flow, and the note goes once it's Matched", async () => {
  const server = alienUnmatched([], admin);
  openApp("/libraries/1/unmatched/7/play", server.transport);

  fireEvent.click(await screen.findByRole("button", { name: "Match it" }));
  fireEvent.click(await screen.findByRole("button", { name: "Match to Alien (1979)" }));
  await waitFor(() => {
    expect(server.setMatches).toEqual([3n]);
  });

  await waitFor(() => {
    expect(screen.queryByText("Not matched, so progress isn’t saved.")).toBeNull();
  });
  expect(screen.getByLabelText("Alien (1979)")).toBeDefined();
});

test("needs a Match plays each Unmatched Item's Version", async () => {
  const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
  const item = { ...ALIEN, itemId: 3n, path: "/srv/films/Alien (1979).mkv" };
  openApp(
    "/admin/needs-a-match",
    fakeServer({ signedIn: admin }, {}, {}, (router) => {
      router.service(LibraryService, { listLibraries: () => ({ libraries: [films] }) });
      router.service(MatchService, { listUnmatchedItems: () => ({ items: [item] }) });
    }),
  );

  const play = await screen.findByRole("link", { name: "Play Alien (1979)" });

  expect(play.getAttribute("href")).toBe("/libraries/1/unmatched/7/play");
});
