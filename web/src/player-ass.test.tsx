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

/** The options of each ASS renderer made, and whether the next one fails to start. */
const libass = vi.hoisted(() => ({
  made: [] as { subContent: string; fonts: string[]; workerUrl: string }[],
  failsToStart: false,
}));
vi.mock("@jellyfin/libass-wasm", () => ({
  default: class {
    constructor(options: {
      subContent: string;
      fonts: string[];
      workerUrl: string;
      onReady: () => void;
      onError: (error: unknown) => void;
    }) {
      libass.made.push(options);
      // As its worker reports a renderer that won't start: after it's made.
      queueMicrotask(
        libass.failsToStart
          ? () => {
              options.onError("no wasm");
            }
          : options.onReady,
      );
    }
    dispose() {}
  },
}));

// happy-dom has no Web Worker, which the ASS renderer needs to be listed.
vi.stubGlobal("Worker", vi.fn());
// happy-dom doesn't name the ready states the player reads.
Object.assign(HTMLMediaElement, { HAVE_METADATA: 1 });

beforeEach(() => {
  Object.assign(libass, { made: [], failsToStart: false });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const STYLED = "/media/versions/7/subtitles/1/ass";
const FONT = "/media/versions/7/fonts/0";
const SIGNS = {
  codec: "ass",
  language: "fas",
  title: "Signs",
  kind: { case: "subtitle", value: {} },
} as const;

/**
 * A fake Server with Alien, whose one Version holds a Persian ASS Track in a file with one font.
 * A Device that lists ASS gets it drawn with its font; any other gets its WebVTT and it to burn
 * in. Each plan request goes in `asked`.
 */
function server(asked: PlanPlaybackRequest[]) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const tracks = [{ codec: "h264", kind: { case: "video", value: {} } } as const, SIGNS];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 348n } },
          title: "Alien",
          versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n, tracks }],
        },
      }),
    });
    router.service(PlaybackService, {
      planPlayback: (request) => {
        asked.push(request);
        const { subtitleTrack, deviceDecoding } = request;
        if (deviceDecoding?.subtitleFormats.includes(SubtitleFormat.ASS) ?? false) {
          const drawn = { track: SIGNS, url: STYLED, format: SubtitleFormat.ASS, fontUrls: [FONT] };
          return { plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/v", subtitles: [drawn] } };
        }
        const plain = {
          track: SIGNS,
          url: "/media/versions/7/subtitles/1",
          format: SubtitleFormat.WEBVTT,
        };
        const plan = { method: PlaybackMethod.TRANSCODE, url: "/p.m3u8", subtitleTrack };
        return { plan: { ...plan, subtitles: [plain], burnInTracks: [1] } };
      },
    });
  });
}

/**
 * Opens the player, a fetch of each URL in `answers` answered by what it gives, as `asked` records
 * its plans, and opens the subtitle menu.
 */
async function playingWith(
  asked: PlanPlaybackRequest[],
  answers: Record<string, () => Promise<Response>>,
) {
  vi.spyOn(globalThis, "fetch").mockImplementation(
    (url) =>
      (typeof url === "string" ? answers[url]?.() : undefined) ?? Promise.resolve(Response.error()),
  );
  openApp("/libraries/1/movies/tmdb-348/play", server(asked));
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  return video;
}

const answer =
  (body: string, status = 200) =>
  () =>
    Promise.resolve(new Response(body, { status }));
const SERVED = { [STYLED]: answer("ASS"), [FONT]: answer("FONT") };

test("a drawn ASS Track is one entry, with no new plan or styling to choose", async () => {
  const asked: PlanPlaybackRequest[] = [];
  await playingWith(asked, SERVED);

  const items = screen.getAllByRole("menuitemradio").map((item) => item.textContent);

  expect(items).toEqual(["Off", "Persian Signs"]);
  expect(asked[0]?.deviceDecoding?.subtitleFormats).toEqual([
    SubtitleFormat.PGS,
    SubtitleFormat.ASS,
  ]);
});

test("an ASS Track says Loading… until it and its fonts are fetched, then is drawn with them", async () => {
  const asked: PlanPlaybackRequest[] = [];
  let sendFont: ((response: Response) => void) | undefined;
  const font = new Promise<Response>((resolve) => {
    sendFont = resolve;
  });
  const video = await playingWith(asked, { ...SERVED, [FONT]: () => font });
  const signs = screen.getByRole("menuitemradio", { name: /Signs/u });

  fireEvent.click(signs);

  await waitFor(() => {
    expect(signs.textContent).toContain("Loading…");
  });
  expect(libass.made).toEqual([]);
  sendFont?.(new Response("FONT"));
  await waitFor(() => {
    expect(signs.textContent).not.toContain("Loading…");
  });
  const [made] = libass.made;
  expect(made?.subContent).toBe("ASS");
  expect(made?.fonts).toHaveLength(1);
  expect(made?.workerUrl).toMatch(/^blob:/u);
  expect(asked).toHaveLength(1);
  expect(screen.getByLabelText("Alien")).toBe(video);
});

test("an ASS Track whose font can't be fetched says so, and isn't drawn", async () => {
  await playingWith([], { ...SERVED, [FONT]: answer("", 404) });
  const signs = screen.getByRole("menuitemradio", { name: /Signs/u });

  fireEvent.click(signs);

  await waitFor(() => {
    expect(signs.textContent).toContain("Couldn’t load");
  });
  expect(libass.made).toEqual([]);
});

// Last: once the renderer fails, this page never lists ASS again, as after it a reload would.
test("a renderer that won't start has the ASS Track burned in by a plan that doesn't list ASS", async () => {
  const asked: PlanPlaybackRequest[] = [];
  libass.failsToStart = true;
  await playingWith(asked, SERVED);

  fireEvent.click(screen.getByRole("menuitemradio", { name: /Signs/u }));

  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked[1]?.subtitleTrack).toBe(1);
  expect(asked[1]?.deviceDecoding?.subtitleFormats).toEqual([SubtitleFormat.PGS]);
});
