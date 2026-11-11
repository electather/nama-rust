import { Code } from "@connectrpc/connect";
import { screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { fakeServer, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

/** A Server already running as many Transcodes as it may, 1, which only a Transcode plays. */
const busy = fakeServer(
  { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
  {},
  {},
  (router) => {
    router.service(PlaybackService, {
      planPlayback: () => {
        throw refused(Code.ResourceExhausted, "the Server is busy", {
          reason: RefusalReason.SERVER_BUSY,
          params: { maxTranscodes: "1" },
        });
      },
    });
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 603n } },
          title: "The Matrix",
          versions: [{ id: 7n, fileName: "The Matrix (1999).mkv", size: 1n }],
        },
      }),
    });
  },
);

test("a Server too busy to Transcode the Version says so in plain words instead of the player", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", busy);

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(
    screen.getByText(
      "The Server is running as many Transcodes as it may at once (1), so it can’t Transcode this until one ends.",
    ),
  ).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});
