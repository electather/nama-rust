import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
  SubtitleFormat,
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
const ENGLISH = "/media/versions/7/subtitles/1";
const JAPANESE = "/media/versions/7/subtitles/3";

/**
 * A fake Server with Alien in Library 1, whose one Version holds an English SRT, an English PGS
 * titled Signs and a Japanese ASS subtitle, the SRT and the ASS offered as text, and the PGS and
 * ASS ones offered to burn in, unless `burnInTracks` says otherwise. It Direct Plays it, or
 * Transcodes it when asked to burn a subtitle in, and each plan request goes in `asked`.
 */
function server(asked: PlanPlaybackRequest[], burnInTracks = [2, 3]) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const text = { case: "subtitle", value: { image: false } } as const;
  const tracks = [
    { codec: "h264", kind: { case: "video", value: {} } },
    { codec: "subrip", language: "eng", kind: text },
    {
      codec: "hdmv_pgs_subtitle",
      language: "eng",
      title: "Signs",
      kind: { case: "subtitle", value: { image: true } },
    },
    { codec: "ass", language: "jpn", kind: text },
    { codec: "aac", default: true, kind: { case: "audio", value: { channels: 2 } } },
  ] as const;
  const subtitles = [
    { track: tracks[1], url: ENGLISH, format: SubtitleFormat.WEBVTT },
    { track: tracks[3], url: JAPANESE, format: SubtitleFormat.WEBVTT },
  ];
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
        const { subtitleTrack } = request;
        const plan =
          subtitleTrack === undefined
            ? { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" }
            : { method: PlaybackMethod.TRANSCODE, url: `/media/plans/${asked.length}/index.m3u8` };
        return { plan: { ...plan, subtitles, subtitleTrack, burnInTracks } };
      },
    });
  });
}

function openSubtitles() {
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  return screen.getAllByRole("menuitemradio");
}

/** Opens the player 754.25 seconds in, as `asked` records its plan requests. */
async function playing(asked: PlanPlaybackRequest[]) {
  openApp(PLAYER, server(asked));
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  video.currentTime = 754.25;
  return video;
}

/** The `<video>` that replaced `before` for a new plan, once it has loaded. */
async function replaced(before: HTMLVideoElement) {
  const video = await waitFor(() => {
    const after = screen.getByLabelText<HTMLVideoElement>("Alien");
    expect(after).not.toBe(before);
    return after;
  });
  // As in a browser, where a new source loads from its start; happy-dom keeps the old time.
  video.currentTime = 0;
  fireEvent(video, new Event("loadedmetadata"));
  return video;
}

test("the picker lists the image subtitles and the ASS ones with their styling after the text ones", async () => {
  openApp(PLAYER, server([]));
  await screen.findByLabelText("Alien");

  const items = openSubtitles();

  expect(items.map((item) => item.textContent)).toEqual([
    "Off",
    "English",
    "Japanese",
    "English Signs",
    "Japanese With styling",
  ]);
});

test("a Server that can’t burn subtitles in offers only the text ones", async () => {
  openApp(PLAYER, server([], []));
  await screen.findByLabelText("Alien");

  expect(openSubtitles().map((item) => item.textContent)).toEqual(["Off", "English", "Japanese"]);
});

test("turning on a PGS Track mid-play asks for a plan from where the video stands with it burned in", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const directPlay = await playing(asked);

  openSubtitles();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Signs/u }));

  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked.map((request) => request.subtitleTrack)).toEqual([undefined, 2]);
  expect(asked[1]?.startPositionMs).toBe(754_250n);
  const transcode = await replaced(directPlay);
  expect(transcode.currentTime).toBe(754.25);
  const checked = openSubtitles().map((item) => item.getAttribute("aria-checked"));
  expect(checked).toEqual(["false", "false", "false", "true", "false"]);
});

test("choosing the ASS Track with its styling burns it in", async () => {
  const asked: PlanPlaybackRequest[] = [];
  await playing(asked);

  openSubtitles();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /With styling/u }));

  await waitFor(() => {
    expect(asked.map((request) => request.subtitleTrack)).toEqual([undefined, 3]);
  });
});

test("a text subtitle chosen while one is burned in asks for a plan without it and shows the text one", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const directPlay = await playing(asked);
  openSubtitles();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Signs/u }));
  const burnedIn = await replaced(directPlay);
  Object.defineProperty(burnedIn, "readyState", { value: HTMLMediaElement.HAVE_METADATA });

  openSubtitles();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /^Japanese$/u }));

  await waitFor(() => {
    expect(asked.map((request) => request.subtitleTrack)).toEqual([undefined, 2, undefined]);
  });
  await replaced(burnedIn);
  const checked = openSubtitles().map((item) => item.getAttribute("aria-checked"));
  expect(checked).toEqual(["false", "false", "true", "false", "false"]);
});
