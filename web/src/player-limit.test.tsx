import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackLimit,
  PlaybackMethod,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";

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

// happy-dom doesn't name the ready states the player reads.
Object.assign(HTMLMediaElement, { HAVE_METADATA: 1 });

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const LANDSCAPE_1080P = { width: 1920, height: 1080 };

/**
 * A fake Server with Alien in Library 1, whose one Version is H.264 with a `picture`, 1080p unless
 * given. It Direct Plays it unless the Playback Limit asked for, or else `remoteDefault`, is below
 * Original, which it Transcodes, and each plan request goes in `asked`.
 */
function server(
  asked: PlanPlaybackRequest[],
  picture = LANDSCAPE_1080P,
  remoteDefault: PlaybackLimit = PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL,
) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const tracks = [
    { codec: "h264", kind: { case: "video", value: picture } },
    { codec: "aac", default: true, kind: { case: "audio", value: { channels: 2 } } },
  ] as const;
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 348n } },
          title: "Alien",
          versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n, tracks: [...tracks] }],
        },
      }),
    });
    router.service(PlaybackService, {
      planPlayback: (request) => {
        asked.push(request);
        const picked = request.playbackLimit;
        const limit = picked === PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED ? remoteDefault : picked;
        const played =
          limit === PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL
            ? { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" }
            : { method: PlaybackMethod.TRANSCODE, url: `/media/plans/${asked.length}/index.m3u8` };
        return { plan: { ...played, playbackLimit: limit } };
      },
    });
  });
}

function openLimits() {
  fireEvent.click(screen.getByRole("button", { name: "Playback Limit" }));
  return screen.getAllByRole("menuitemradio");
}

/** Plays `video` on from `seconds` in, as far as the player can tell. */
function playingAt(video: HTMLVideoElement, seconds: number) {
  Object.defineProperty(video, "readyState", {
    value: HTMLMediaElement.HAVE_METADATA,
    configurable: true,
  });
  video.currentTime = seconds;
}

test("on a 1080p Version the Playback Limit menu offers Original, 720p and 480p, Original chosen", async () => {
  openApp(PLAYER, server([]));
  await screen.findByLabelText("Alien");

  const items = openLimits();

  expect(items.map((item) => item.textContent)).toEqual(["Original", "720p", "480p"]);
  expect(items.map((item) => item.getAttribute("aria-checked"))).toEqual([
    "true",
    "false",
    "false",
  ]);
});

test("on a portrait 1080p Version the Playback Limit menu offers Original, 720p and 480p", async () => {
  openApp(PLAYER, server([], { width: 1080, height: 1920 }));
  await screen.findByLabelText("Alien");

  expect(openLimits().map((item) => item.textContent)).toEqual(["Original", "720p", "480p"]);
});

test("choosing 720p mid-play, then Original, asks each time for a plan from where the video stands", async () => {
  const asked: PlanPlaybackRequest[] = [];
  openApp(PLAYER, server(asked));
  const original = await screen.findByLabelText<HTMLVideoElement>("Alien");
  playingAt(original, 754.25);

  openLimits();
  fireEvent.click(screen.getByRole("menuitemradio", { name: "720p" }));

  const limited = await waitFor(() => {
    const video = screen.getByLabelText<HTMLVideoElement>("Alien");
    expect(video).not.toBe(original);
    return video;
  });
  expect(asked[1]?.playbackLimit).toBe(PlaybackLimit.PLAYBACK_LIMIT_720P);
  expect(asked[1]?.startPositionMs).toBe(754_250n);
  // As in a browser, where a new source loads from its start; happy-dom keeps the old time.
  limited.currentTime = 0;
  fireEvent(limited, new Event("loadedmetadata"));
  expect(limited.currentTime).toBe(754.25);
  expect(openLimits().map((item) => item.getAttribute("aria-checked"))).toEqual([
    "false",
    "true",
    "false",
  ]);
  playingAt(limited, 800);

  fireEvent.click(screen.getByRole("menuitemradio", { name: "Original" }));

  await waitFor(() => {
    expect(asked).toHaveLength(3);
  });
  expect(asked[2]?.playbackLimit).toBe(PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL);
  expect(asked[2]?.startPositionMs).toBe(800_000n);
});

test("away from home the menu shows the remote default the Server chose, until Original is picked", async () => {
  const asked: PlanPlaybackRequest[] = [];
  openApp(PLAYER, server(asked, LANDSCAPE_1080P, PlaybackLimit.PLAYBACK_LIMIT_720P));
  await screen.findByLabelText("Alien");

  const shown = openLimits().map((item) => item.getAttribute("aria-checked"));
  fireEvent.click(screen.getByRole("menuitemradio", { name: "Original" }));

  expect(asked[0]?.playbackLimit).toBe(PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED);
  expect(shown).toEqual(["false", "true", "false"]);
  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked[1]?.playbackLimit).toBe(PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL);
});

test("a remote default the Version already fits shows as Original", async () => {
  openApp(PLAYER, server([], { width: 1280, height: 720 }, PlaybackLimit.PLAYBACK_LIMIT_1080P));
  await screen.findByLabelText("Alien");

  expect(openLimits().map((item) => item.getAttribute("aria-checked"))).toEqual(["true", "false"]);
});
