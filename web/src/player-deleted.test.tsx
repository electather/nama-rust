import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { eventFeed, fakeServer, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

/**
 * A Server whose Library 1 has The Matrix, Direct Played, until someone `deletes` it. From then on
 * it refuses the Movie and its Version as the real Server refuses ones that are gone, and
 * `tellsTheApp` sends the Event that says so.
 */
function matrixUntilDeleted() {
  const events = eventFeed();
  let deleted = false;
  const transport = fakeServer(
    {
      signedIn: newUser,
      libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }],
      events: events.stream,
    },
    {},
    {},
    (router) => {
      router.service(PlaybackService, {
        planPlayback: () => {
          if (deleted) {
            throw refused(Code.NotFound, "there is no such Version", {
              reason: RefusalReason.VERSION_NOT_FOUND,
            });
          }
          return { plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" } };
        },
      });
      router.service(BrowseService, {
        getMovie: () => {
          if (deleted) {
            throw refused(Code.NotFound, "nothing is matched to that title", {
              reason: RefusalReason.NOTHING_MATCHED,
            });
          }
          return {
            movie: {
              title: "The Matrix",
              versions: [{ id: 7n, fileName: "Matrix.mkv", size: 1n }],
            },
          };
        },
      });
    },
  );
  return {
    transport,
    deletes: () => {
      deleted = true;
    },
    tellsTheApp: () => {
      events.send({ event: { case: "libraryItemRemoved", value: { libraryId: 1n, itemId: 1n } } });
    },
  };
}

test("the player says what it plays was deleted once an Event says it's gone", async () => {
  const server = matrixUntilDeleted();
  openApp("/libraries/1/movies/tmdb-603/play", server.transport);
  await screen.findByLabelText("The Matrix");

  server.deletes();
  server.tellsTheApp();

  expect(await screen.findByRole("heading", { name: "Deleted from the Server" })).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});

test("a player that fails as its Version is deleted, before the Event comes, says it was deleted", async () => {
  const server = matrixUntilDeleted();
  openApp("/libraries/1/movies/tmdb-603/play", server.transport);
  const video = await screen.findByLabelText("The Matrix");

  server.deletes();
  fireEvent.error(video);

  expect(await screen.findByRole("heading", { name: "Deleted from the Server" })).toBeDefined();
});
