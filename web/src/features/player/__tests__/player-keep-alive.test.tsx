import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackLimit, PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { pickIn } from "@/test-player";

// oxlint-disable-next-line vitest/prefer-import-in-mock -- the fake has only what the player calls, so it can't be typed as the module.
vi.mock("hls.js", () => ({
  default: class {
    public static Events = { ERROR: "hlsError" };
    public on = vi.fn<() => void>();
    public loadSource = vi.fn<() => void>();
    public attachMedia = vi.fn<() => void>();
    public destroy = vi.fn<() => void>();
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
 * A Server whose first plan for The Matrix plays from `url` by `method`, and whose plan once that
 * fails is a Remux, each offering a 720p Playback Limit, and numbered from 1 in the order asked.
 * The id of each plan the player says it stopped goes in `stopped`, and for each plan asked for,
 * those stopped by then go in `stoppedWhenAsked`.
 */
// oxlint-disable-next-line max-params -- the tests read as the Server they set up, in this order
const planning = (
  method: PlaybackMethod,
  url: string,
  stopped: bigint[] = [],
  stoppedWhenAsked: bigint[][] = [],
) =>
  fakeServer(
    { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
    {},
    {},
    (router) => {
      router.service(PlaybackService, {
        planPlayback: ({ failedRecipes }) => {
          stoppedWhenAsked.push([...stopped]);
          const id = BigInt(stoppedWhenAsked.length);
          const played =
            failedRecipes.length === 0
              ? { method, url }
              : { method: PlaybackMethod.REMUX, url: PLAYLIST };
          return { plan: { id, ...played, playbackLimits: [PlaybackLimit.PLAYBACK_LIMIT_720P] } };
        },
        stopPlayback: ({ planId }) => {
          stopped.push(planId);
          return {};
        },
      });
      router.service(BrowseService, {
        getMovie: () => ({
          movie: {
            id: { namespace: "tmdb", value: "603" },
            title: "The Matrix",
            versions: [{ id: 7n, fileName: "The Matrix (1999).mkv", size: 1n }],
          },
        }),
      });
    },
  );

/** Waits long enough for a stop or a new ask the player sent to reach the Server. */
async function afterAWhile() {
  await new Promise((resolve) => {
    setTimeout(resolve, 50);
  });
}

/** Chooses the 720p Playback Limit in the player's menu. */
function choose720p() {
  pickIn("Playback Limit", "720p");
}

/** Waits for the player to play a new plan in a `<video>` other than `before`. */
async function newVideo(before: HTMLElement) {
  await waitFor(() => {
    expect(screen.getByLabelText("The Matrix")).not.toBe(before);
  });
}

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

  await afterAWhile();

  expect(stopped).toStrictEqual([]);
});

test("the Server is told a plan stopped once a new plan replaces it, and once the player closes", async () => {
  const stopped: bigint[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", planning(PlaybackMethod.DIRECT_PLAY, FILE, stopped));

  const first = await screen.findByLabelText("The Matrix");
  fireEvent.error(first);
  await waitFor(() => {
    expect(stopped).toStrictEqual([1n]);
  });
  await newVideo(first);
  cleanup();

  await waitFor(() => {
    expect(stopped).toStrictEqual([1n, 2n]);
  });
});

// So at the Transcode limit the new plan finds the old one's room.
test("choosing a Playback Limit tells the Server the plan stopped before asking for the new one", async () => {
  const stopped: bigint[] = [];
  const stoppedWhenAsked: bigint[][] = [];
  openApp(
    "/libraries/1/movies/tmdb-603/play",
    planning(PlaybackMethod.REMUX, PLAYLIST, stopped, stoppedWhenAsked),
  );
  const first = await screen.findByLabelText("The Matrix");

  choose720p();

  await waitFor(() => {
    expect(stoppedWhenAsked).toStrictEqual([[], [1n]]);
  });
  await newVideo(first);
  cleanup();
  await waitFor(() => {
    expect(stopped).toStrictEqual([1n, 2n]);
  });
});

test("the plan being replaced failing once the Server stops it asks for nothing more", async () => {
  const stoppedWhenAsked: bigint[][] = [];
  openApp(
    "/libraries/1/movies/tmdb-603/play",
    planning(PlaybackMethod.REMUX, PLAYLIST, [], stoppedWhenAsked),
  );
  const first = await screen.findByLabelText("The Matrix");

  choose720p();
  fireEvent.error(first);
  await newVideo(first);

  expect(stoppedWhenAsked).toHaveLength(2);
});

// A reloaded tab no longer knows its old plan, which would hold its Transcode for a minute.
test("leaving the page tells the Server the plan stopped", async () => {
  const stopped: bigint[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", planning(PlaybackMethod.REMUX, PLAYLIST, stopped));
  await screen.findByLabelText("The Matrix");

  fireEvent(globalThis.window, new Event("pagehide"));

  await waitFor(() => {
    expect(stopped).toStrictEqual([1n]);
  });
});

// It may come back from the back/forward cache, still playing the plan.
test("a page kept to come back to leaves the plan running", async () => {
  const stopped: bigint[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", planning(PlaybackMethod.REMUX, PLAYLIST, stopped));
  await screen.findByLabelText("The Matrix");

  fireEvent(globalThis.window, Object.assign(new Event("pagehide"), { persisted: true }));
  await afterAWhile();

  expect(stopped).toStrictEqual([]);
});

test("seeking neither asks for a new plan nor stops the one playing", async () => {
  const stopped: bigint[] = [];
  const stoppedWhenAsked: bigint[][] = [];
  openApp(
    "/libraries/1/movies/tmdb-603/play",
    planning(PlaybackMethod.REMUX, PLAYLIST, stopped, stoppedWhenAsked),
  );
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");

  video.currentTime = 600;
  fireEvent(video, new Event("seeked"));
  await afterAWhile();

  expect(stoppedWhenAsked).toStrictEqual([[]]);
  expect(stopped).toStrictEqual([]);
});
