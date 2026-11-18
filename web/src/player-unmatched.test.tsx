import { Code } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { MatchService, TitleKind } from "@/gen/nama/v1/match_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { admin, eventFeed, fakeServer, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];

const ALIEN = { versionId: 7n, kind: TitleKind.MOVIE, title: "Alien", year: 1979 };

/**
 * A fake Server whose Library 1 lists Alien as Unmatched and plays its Version 7, until `matches`
 * Matches it: from then on it refuses Alien's Version as Unmatched, counting each refusal in
 * `refusals`, and `tellsTheApp` sends the Event that says so. Each Version it plans goes in
 * `planned`.
 */
function alienUnmatched(planned: bigint[]) {
  const events = eventFeed();
  let matched = false;
  let refusals = 0;
  const transport = fakeServer(
    { signedIn: newUser, libraries, events: events.stream },
    {},
    {},
    (router) => {
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
    },
  );
  return {
    transport,
    matches: () => {
      matched = true;
    },
    tellsTheApp: () => {
      events.send({ event: { case: "itemMatched", value: { libraryId: 1n, itemId: 1n } } });
    },
    refusals: () => refusals,
  };
}

test("Play on a Not matched Item plays its Version, titled by what its name says", async () => {
  const planned: bigint[] = [];
  openApp("/libraries/1", alienUnmatched(planned).transport);

  fireEvent.click(await screen.findByRole("link", { name: "Play Alien (1979)" }));

  expect(await screen.findByLabelText("Alien (1979)")).toBeDefined();
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
    await new Promise((settled) => {
      setTimeout(settled, 20);
    });
  });

  expect(screen.getByLabelText("Alien (1979)")).toBeDefined();
});

test("Needs a Match plays each Unmatched Item's Version", async () => {
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
