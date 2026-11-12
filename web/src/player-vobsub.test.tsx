import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
  SubtitleFormat,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

/**
 * What each VobSub renderer drew, as text, under which file name, whether the next one fails to
 * start, and how many ended.
 */
const bitsub = vi.hoisted(() => ({
  drawn: [] as { content: string; fileName: string }[],
  failsToStart: false,
  disposed: 0,
}));
vi.mock("libbitsub", () => ({
  VobSubRenderer: class {
    constructor({
      subContent,
      fileName,
      onLoaded,
      onEvent,
    }: {
      subContent: ArrayBuffer;
      fileName: string;
      onLoaded: () => void;
      onEvent: (event: { type: string }) => void;
    }) {
      // As libbitsub reports a renderer that won't start: an event, after it's made.
      if (bitsub.failsToStart) {
        queueMicrotask(() => {
          onEvent({ type: "error" });
        });
        return;
      }
      bitsub.drawn.push({ content: new TextDecoder().decode(subContent), fileName });
      onLoaded();
    }
    dispose() {
      bitsub.disposed += 1;
    }
  },
}));

// happy-dom doesn't name the ready states the player reads.
Object.assign(HTMLMediaElement, { HAVE_METADATA: 1 });

beforeEach(() => {
  Object.assign(bitsub, { drawn: [], failsToStart: false, disposed: 0 });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const ENGLISH = "/media/versions/7/subtitles/1";
const DUTCH = "/media/versions/7/subtitles/2";

/**
 * A fake Server with Alien in Library 1, whose one Version holds an English SRT and a Dutch VobSub
 * that carries its header. A Device that lists VobSub gets both among its plan's subtitles; any
 * other gets the VobSub one to burn in, and a Transcode when it asks for that. Each plan request
 * goes in `asked`.
 */
function server(asked: PlanPlaybackRequest[]) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const tracks = [
    { codec: "h264", kind: { case: "video", value: {} } },
    { codec: "subrip", language: "eng", kind: { case: "subtitle", value: {} } },
    { codec: "dvd_subtitle", language: "dut", kind: { case: "subtitle", value: { image: true } } },
  ] as const;
  const english = { track: tracks[1], url: ENGLISH, place: 1, format: SubtitleFormat.WEBVTT };
  const dutch = { track: tracks[2], url: DUTCH, place: 2, format: SubtitleFormat.VOBSUB };
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
        const { subtitleTrack, deviceDecoding } = request;
        const draws = deviceDecoding?.subtitleFormats.includes(SubtitleFormat.VOBSUB) ?? false;
        const plan =
          subtitleTrack === undefined
            ? { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" }
            : { method: PlaybackMethod.TRANSCODE, url: "/media/plans/1/index.m3u8" };
        return draws
          ? { plan: { ...plan, subtitles: [english, dutch] } }
          : { plan: { ...plan, subtitles: [english], burnInTracks: [2], subtitleTrack } };
      },
    });
  });
}

/** Opens the player on [`server`], its subtitles fetched as `MKS`, as `asked` records its plans. */
async function playing(asked: PlanPlaybackRequest[]) {
  vi.spyOn(globalThis, "fetch").mockImplementation(() => Promise.resolve(new Response("MKS")));
  openApp(PLAYER, server(asked));
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  return video;
}

function choose(name: RegExp) {
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  fireEvent.click(screen.getByRole("menuitemradio", { name }));
}

test("switching between a VobSub Track, a text one and Off draws it as Matroska and doesn't restart playback", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const video = await playing(asked);
  choose(/Dutch/u);
  await waitFor(() => {
    expect(bitsub.drawn).toEqual([{ content: "MKS", fileName: "track.mks" }]);
  });
  expect(vi.mocked(fetch)).toHaveBeenCalledWith(DUTCH);

  fireEvent.click(screen.getByRole("menuitemradio", { name: /English/u }));
  await waitFor(() => {
    expect(bitsub.disposed).toBe(1);
  });
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Dutch/u }));
  await waitFor(() => {
    expect(bitsub.drawn).toHaveLength(2);
  });
  fireEvent.click(screen.getByRole("menuitemradio", { name: "Off" }));

  await waitFor(() => {
    expect(bitsub.disposed).toBe(2);
  });
  expect(asked).toHaveLength(1);
  expect(asked[0]?.deviceDecoding?.subtitleFormats).toContain(SubtitleFormat.VOBSUB);
  expect(screen.getByLabelText("Alien")).toBe(video);
});

// Last: once the renderer fails, this page never lists VobSub again, as after it a reload would.
test("a renderer that won't start has the VobSub Track burned in by a plan that doesn't list VobSub", async () => {
  const asked: PlanPlaybackRequest[] = [];
  bitsub.failsToStart = true;
  const directPlay = await playing(asked);

  choose(/Dutch/u);

  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked[0]?.deviceDecoding?.subtitleFormats).toContain(SubtitleFormat.VOBSUB);
  expect(asked[1]?.subtitleTrack).toBe(2);
  expect(asked[1]?.deviceDecoding?.subtitleFormats).not.toContain(SubtitleFormat.VOBSUB);
  await waitFor(() => {
    expect(screen.getByLabelText("Alien")).not.toBe(directPlay);
  });
});
