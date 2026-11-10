import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

vi.mock("hls.js", () => ({
  default: class {
    static Events = { ERROR: "hlsError" };
    on() {}
    loadSource() {}
    attachMedia() {}
    destroy() {}
  },
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const PLAYLIST = "/media/plans/0/index.m3u8";

const remuxing = fakeServer(
  { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
  {},
  {},
  (router) => {
    router.service(PlaybackService, {
      planPlayback: () => ({ plan: { method: PlaybackMethod.REMUX, url: PLAYLIST } }),
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

// The Server stops a plan a minute after it was last asked for, and a paused player asks for nothing.
test("an open Remux asks for its playlist every 30 seconds, so a pause doesn't stop it", async () => {
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
  const fetched = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response());
  openApp("/libraries/1/movies/tmdb-603/play", remuxing);
  await screen.findByLabelText("The Matrix");
  const playlistAsks = () => fetched.mock.calls.filter(([url]) => url === PLAYLIST);
  const before = playlistAsks().length;

  vi.advanceTimersByTime(30_000);

  expect(playlistAsks()).toHaveLength(before + 1);
});
