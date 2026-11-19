import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService, SubtitleFormat } from "@/gen/nama/v1/playback_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openSettingsAt, openTracks, rowIn } from "@/test-player";

const SHOW = "/libraries/2/shows/tmdb-70523";
const DURATION_S = 1000;
const ENGLISH = "/media/versions/3/subtitles/1";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function episode(episodeNumber: number, title: string) {
  return {
    id: { namespace: "tmdb", value: String(episodeNumber) },
    seasonNumber: 1,
    episodeNumber,
    title,
    versions: [
      {
        id: BigInt(episodeNumber),
        fileName: `Dark S01E0${episodeNumber}.mkv`,
        size: 1n,
        tracks: [{ codec: "h264", kind: { case: "video", value: { frameRate: 25 } } } as const],
      },
    ],
  };
}

/** A fake Server with Dark's Episode 3, at 25 frames a second with English subtitles, then 4. */
const server = fakeServer(
  { signedIn: newUser, libraries: [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }] },
  {},
  {},
  (router) => {
    router.service(BrowseService, {
      getShow: () => ({ show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" } }),
      listEpisodes: () => ({
        episodes: [episode(3, "Past and Present"), episode(4, "Double Lives")],
      }),
      getNextEpisode: ({ episodeNumber }) =>
        episodeNumber === 3 ? { next: episode(4, "Double Lives") } : {},
    });
    router.service(PlaybackService, {
      getAutoplay: () => ({ on: false }),
      planPlayback: ({ versionId }) => ({
        plan: {
          method: PlaybackMethod.DIRECT_PLAY,
          url: `/media/versions/${versionId}`,
          subtitles: [
            {
              track: { codec: "subrip", language: "eng", kind: { case: "subtitle", value: {} } },
              url: ENGLISH,
              format: SubtitleFormat.WEBVTT,
              place: 1,
            },
          ],
        },
      }),
    });
    router.service(WatchStateService, { reportProgress: () => ({}) });
  },
);

/** Opens Episode 3's player, its `<video>` 100 seconds into 1000, once the next Episode is known. */
async function playEpisode3() {
  const router = openApp(`${SHOW}/seasons/1/episodes/3/play`, server);
  const video = await screen.findByLabelText<HTMLVideoElement>("Past and Present");
  Object.defineProperty(video, "duration", { value: DURATION_S });
  video.currentTime = 100;
  fireEvent(video, new Event("durationchange"));
  await waitFor(() => {
    expect(router.state.status).toBe("idle");
  });
  return { router, video };
}

/** The speed the Speed view shows, as "1.00×". */
function speed() {
  return screen.getByRole("status").textContent;
}

function press(key: string, shiftKey = false) {
  fireEvent.keyDown(document, { key, shiftKey });
}

describe("keys seek as YouTube's do", () => {
  test.each([
    ["ArrowLeft", 95],
    ["ArrowRight", 105],
    ["j", 90],
    ["l", 110],
    ["Home", 0],
    ["0", 0],
    ["4", 400],
    ["9", 900],
    ["End", DURATION_S],
  ])("%s seeks to %ds", async (key, seconds) => {
    const { video } = await playEpisode3();

    press(key);

    expect(video.currentTime).toBe(seconds);
  });
});

test("k and Space play and pause, and Caps Lock doesn't change K", async () => {
  const { video } = await playEpisode3();
  video.pause();

  press("k");
  expect(video.paused).toBe(false);
  press("K");
  expect(video.paused).toBe(true);
  press(" ");
  expect(video.paused).toBe(false);
});

test("comma and period step one frame, by the file's frame rate, only while paused", async () => {
  const { video } = await playEpisode3();
  video.pause();

  press(".");
  expect(video.currentTime).toBe(100.04);
  press(",");
  expect(video.currentTime).toBe(100);

  press("k");
  press(".");
  expect(video.currentTime).toBe(100);
});

test("< and > step the speed to the next quarter, stopping at 0.25×", async () => {
  const { video } = await playEpisode3();
  video.playbackRate = 1.35;

  press(">", true);
  expect(video.playbackRate).toBe(1.5);
  press("<", true);
  press("<", true);
  expect(video.playbackRate).toBe(1);
  for (const _ of [1, 2, 3, 4, 5]) {
    press("<", true);
  }
  expect(video.playbackRate).toBe(0.25);
});

test("the arrows up and down step the volume by 5%, and M mutes", async () => {
  const { video } = await playEpisode3();
  video.volume = 0.5;

  press("ArrowUp");
  expect(video.volume).toBe(0.55);
  press("ArrowDown");
  press("ArrowDown");
  expect(video.volume).toBe(0.45);
  press("m");
  expect(video.muted).toBe(true);
  press("m");
  expect(video.muted).toBe(false);
});

test("c turns the first subtitle on, and off again", async () => {
  await playEpisode3();

  press("c");
  openTracks();
  expect(rowIn("Subtitles", /English/u).getAttribute("aria-checked")).toBe("true");

  // Keys are the panel's while it's open, so C waits for it to close.
  press("Escape");
  await waitFor(() => {
    expect(screen.queryByRole("dialog", { name: "Audio and subtitles" })).toBeNull();
  });
  press("c");
  openTracks();
  expect(rowIn("Subtitles", "Off").getAttribute("aria-checked")).toBe("true");
});

test("shift+N plays the next Episode", async () => {
  const { router } = await playEpisode3();

  press("N", true);

  await screen.findByLabelText("Double Lives");
  expect(router.state.location.pathname).toBe(`${SHOW}/seasons/1/episodes/4/play`);
});

test("the Next Track media key plays the next Episode", async () => {
  const handlers = new Map<MediaSessionAction, MediaSessionActionHandler | null>();
  Object.defineProperty(navigator, "mediaSession", {
    value: {
      setActionHandler: (action: MediaSessionAction, handler: MediaSessionActionHandler | null) => {
        handlers.set(action, handler);
      },
      setPositionState: () => {},
    },
    configurable: true,
  });
  vi.stubGlobal("MediaMetadata", vi.fn());
  await playEpisode3();

  act(() => {
    handlers.get("nexttrack")?.({ action: "nexttrack" });
  });

  await expect(screen.findByLabelText("Double Lives")).resolves.toBeDefined();
  Reflect.deleteProperty(navigator, "mediaSession");
});

test("speed shows the speed, which the presets, − and + set, keeping the panel open", async () => {
  const { video } = await playEpisode3();
  openSettingsAt("Speed");
  expect(speed()).toBe("1.00×");

  fireEvent.click(screen.getByRole("button", { name: "1.5" }));
  expect(video.playbackRate).toBe(1.5);
  fireEvent(video, new Event("ratechange"));
  expect(speed()).toBe("1.50×");
  expect(screen.getByRole("button", { name: "1.5" }).getAttribute("aria-pressed")).toBe("true");

  fireEvent.click(screen.getByRole("button", { name: "Faster" }));
  expect(video.playbackRate).toBe(1.55);
  fireEvent.click(screen.getByRole("button", { name: "Slower" }));
  fireEvent.click(screen.getByRole("button", { name: "Slower" }));
  expect(video.playbackRate).toBe(1.45);
  expect(screen.getByRole("dialog", { name: "Playback settings" })).toBeDefined();
});
