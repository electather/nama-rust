import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
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

const PLAYLIST = "/media/plans/2/index.m3u8";
const FILE = "/media/versions/7";

/**
 * A Server whose first plan for The Matrix is plan 1, playing from `url` by `method`, and whose
 * plan once that fails is plan 2, a Remux. The id of each plan the player says it stopped goes in
 * `stopped`.
 */
const planning = (method: PlaybackMethod, url: string, stopped: bigint[] = []) =>
  fakeServer(
    { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
    {},
    {},
    (router) => {
      router.service(PlaybackService, {
        planPlayback: ({ failedMethods }) => ({
          plan:
            failedMethods.length === 0
              ? { id: 1n, method, url }
              : { id: 2n, method: PlaybackMethod.REMUX, url: PLAYLIST },
        }),
        stopPlayback: ({ planId }) => {
          stopped.push(planId);
          return {};
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

// The Server stops a plan a minute after it was last asked for, and a paused player asks for nothing.
test.each([
  ["Remux", PlaybackMethod.REMUX, PLAYLIST],
  ["Direct Play", PlaybackMethod.DIRECT_PLAY, FILE],
])(
  "an open %s asks for its URL's headers every 30 seconds, so a pause doesn't stop it",
  async (_, method, url) => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const fetched = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response());
    openApp("/libraries/1/movies/tmdb-603/play", planning(method, url));
    await screen.findByLabelText("The Matrix");
    const asks = () => fetched.mock.calls.filter(([asked]) => asked === url);
    const before = asks().length;

    vi.advanceTimersByTime(30_000);

    expect(asks()).toHaveLength(before + 1);
    expect(fetched).toHaveBeenLastCalledWith(url, { method: "HEAD" });
  },
);

test("a plan its player still plays is never stopped, though React sets up its effects twice", async () => {
  const stopped: bigint[] = [];
  openApp(
    "/libraries/1/movies/tmdb-603/play",
    planning(PlaybackMethod.REMUX, PLAYLIST, stopped),
    (app) => <StrictMode>{app}</StrictMode>,
  );
  await screen.findByLabelText("The Matrix");

  // Long enough for a stop sent on the effect's first cleanup to arrive.
  await new Promise((resolve) => {
    setTimeout(resolve, 50);
  });

  expect(stopped).toEqual([]);
});

test("the Server is told a plan stopped once a new plan replaces it, and once the player closes", async () => {
  const stopped: bigint[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", planning(PlaybackMethod.DIRECT_PLAY, FILE, stopped));

  fireEvent.error(await screen.findByLabelText("The Matrix"));
  await waitFor(() => {
    expect(stopped).toEqual([1n]);
  });
  cleanup();

  await waitFor(() => {
    expect(stopped).toEqual([1n, 2n]);
  });
});
