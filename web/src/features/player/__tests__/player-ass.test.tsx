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
import { fakeServer, newUser, openApp } from "@/test-app";
import { openTracks, rowIn, rowsOf } from "@/test-player";
import { tickWorker } from "@/test-worker";

/** The options of each ASS renderer made, each renderer, and whether the next one fails to start. */
const libass = vi.hoisted(
  (): {
    made: { subContent: string; fonts: Uint8Array[] }[];
    renderers: { timeOffset: number; resize: () => Promise<void> }[];
    failsToStart: boolean;
  } => ({ made: [], renderers: [], failsToStart: false }),
);
// oxlint-disable-next-line vitest/prefer-import-in-mock -- the fake has only what the player calls, so it can't be typed as the module.
vi.mock("jassub", () => ({
  default: class {
    public readonly ready: Promise<void>;
    public timeOffset = 0;
    public resize = vi.fn<() => Promise<void>>(async () => {});
    public readonly _worker = Object.assign(new EventTarget(), { terminate: vi.fn<() => void>() });
    public constructor(options: { subContent: string; fonts: Uint8Array[] }) {
      libass.made.push(options);
      libass.renderers.push(this);
      // As its worker reports a track libass rejects.
      this.ready = libass.failsToStart ? Promise.reject(new Error("no track")) : Promise.resolve();
    }
    // As the package's does, which waits for it to be ready, so rethrows a failed start.
    public async destroy() {
      await this.ready;
    }
  },
}));

/** A Worker that reads `type` as a browser with module workers does. */
function moduleWorker(_url: string, options?: WorkerOptions) {
  void options?.type;
  return tickWorker();
}

beforeEach(() => {
  vi.stubGlobal("Worker", moduleWorker);
  // happy-dom doesn't name the ready states the player reads.
  Object.assign(HTMLMediaElement, { HAVE_METADATA: 1 });
  Object.assign(libass, { made: [], renderers: [], failsToStart: false });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
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
          id: { namespace: "tmdb", value: "348" },
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
          const drawn = {
            track: SIGNS,
            url: STYLED,
            place: 1,
            format: SubtitleFormat.ASS,
            fontUrls: [FONT],
            offsetMs: 2000,
          };
          return { plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/v", subtitles: [drawn] } };
        }
        const plain = {
          track: SIGNS,
          url: "/media/versions/7/subtitles/1",
          place: 1,
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
    async (url) =>
      (typeof url === "string" ? await answers[url]?.() : undefined) ?? Response.error(),
  );
  openApp("/libraries/1/movies/tmdb-348/play", server(asked));
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  openTracks();
  return video;
}

const answer =
  (body: string, status = 200) =>
  // oxlint-disable-next-line require-await -- a fake fetch answers at once
  async () =>
    new Response(body, { status });
const SERVED = { [STYLED]: answer("ASS"), [FONT]: answer("FONT") };

test("a drawn ASS Track is one entry, with no new plan or styling to choose", async () => {
  const asked: PlanPlaybackRequest[] = [];
  await playingWith(asked, SERVED);

  const items = rowsOf("Subtitles").map((item) => item.textContent);

  expect(items).toStrictEqual(["Off", "Persian Signs"]);
  expect(asked[0]?.deviceDecoding?.subtitleFormats).toStrictEqual([
    SubtitleFormat.PGS,
    SubtitleFormat.VOBSUB,
    SubtitleFormat.ASS,
  ]);
});

test("an ASS Track says Loading… until it and its fonts are fetched, then is drawn with them", async () => {
  const asked: PlanPlaybackRequest[] = [];
  let sendFont: ((response: Response) => void) | undefined;
  const font = new Promise<Response>((resolve) => {
    sendFont = resolve;
  });
  // oxlint-disable-next-line require-await -- the font arrives when the test sends it
  const video = await playingWith(asked, { ...SERVED, [FONT]: async () => font });
  const signs = rowIn("Subtitles", /Signs/u);

  fireEvent.click(signs);

  await waitFor(() => {
    expect(signs.textContent).toContain("Loading…");
  });
  expect(libass.made).toStrictEqual([]);
  sendFont?.(new Response("FONT"));
  await waitFor(() => {
    expect(signs.textContent).not.toContain("Loading…");
  });
  const [made] = libass.made;
  expect(made?.subContent).toBe("ASS");
  expect(made?.fonts).toHaveLength(1);
  expect(asked).toHaveLength(1);
  expect(screen.getByLabelText("Alien")).toBe(video);
});

test("an ASS Track is drawn at its offset, and a nudge shifts and redraws it in place", async () => {
  await playingWith([], SERVED);
  fireEvent.click(rowIn("Subtitles", /Signs/u));
  await waitFor(() => {
    expect(libass.renderers.map(({ timeOffset }) => timeOffset)).toStrictEqual([-2]);
  });

  fireEvent.click(screen.getByRole("button", { name: "Show earlier" }));

  await waitFor(() => {
    expect(libass.renderers.map(({ timeOffset }) => timeOffset)).toStrictEqual([-1.9]);
  });
  // A paused video draws no next frame of its own.
  expect(libass.renderers[0]?.resize).toHaveBeenCalledWith();
});

test("an ASS Track whose font can't be fetched says so, and isn't drawn", async () => {
  await playingWith([], { ...SERVED, [FONT]: answer("", 404) });
  const signs = rowIn("Subtitles", /Signs/u);

  fireEvent.click(signs);

  await waitFor(() => {
    expect(signs.textContent).toContain("Couldn’t load");
  });
  expect(libass.made).toStrictEqual([]);
});

test.each([
  // A constructor without the method.
  { lacking: "OffscreenCanvas", global: "HTMLCanvasElement", stub: vi.fn<() => void>() },
  // A Worker that never reads `type`.
  { lacking: "module workers", global: "Worker", stub: tickWorker },
])("a Device without $lacking doesn't list ASS", async ({ global, stub }) => {
  vi.stubGlobal(global, stub);
  const asked: PlanPlaybackRequest[] = [];

  await playingWith(asked, SERVED);

  expect(asked[0]?.deviceDecoding?.subtitleFormats).toStrictEqual([
    SubtitleFormat.PGS,
    SubtitleFormat.VOBSUB,
  ]);
});

// Last: once the renderer fails, this page never lists ASS again, as after it a reload would.
test("a renderer that won't start has the ASS Track burned in by a plan that doesn't list ASS", async () => {
  const asked: PlanPlaybackRequest[] = [];
  libass.failsToStart = true;
  const directPlay = await playingWith(asked, SERVED);

  fireEvent.click(rowIn("Subtitles", /Signs/u));

  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked[1]?.subtitleTrack).toBe(1);
  expect(asked[1]?.deviceDecoding?.subtitleFormats).toStrictEqual([
    SubtitleFormat.PGS,
    SubtitleFormat.VOBSUB,
  ]);
  await waitFor(() => {
    expect(screen.getByLabelText("Alien")).not.toBe(directPlay);
  });
});
