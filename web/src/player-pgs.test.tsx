import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import type { MessageInitShape } from "@bufbuild/protobuf";

import { BrowseService, type TrackSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
  SubtitleFormat,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

/** What each PGS renderer drew, as text, whether the next one fails to start, and how many ended. */
const bitsub = vi.hoisted(() => ({ drawn: [] as string[], failsToStart: false, disposed: 0 }));
vi.mock("libbitsub", () => ({
  PgsRenderer: class {
    constructor({
      subContent,
      onLoaded,
      onEvent,
    }: {
      subContent: ArrayBuffer;
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
      bitsub.drawn.push(new TextDecoder().decode(subContent));
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
const SIGNS = "/media/versions/7/subtitles/2";

const PGS_SIGNS = {
  codec: "hdmv_pgs_subtitle",
  language: "eng",
  title: "Signs",
  kind: { case: "subtitle", value: { image: true } },
} as const;

/**
 * A fake Server with Alien in Library 1, whose one Version holds an English SRT and an English PGS
 * titled Signs, then `twins` more PGS Tracks just like it. A Device that lists PGS gets them all
 * among its plan's subtitles; any other gets the PGS ones to burn in, and a Transcode when it asks
 * for that. Each plan request goes in `asked`.
 */
function server(asked: PlanPlaybackRequest[], twins = 0) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const tracks: MessageInitShape<typeof TrackSchema>[] = [
    { codec: "h264", kind: { case: "video", value: {} } },
    { codec: "subrip", language: "eng", kind: { case: "subtitle", value: {} } },
    ...Array.from({ length: 1 + twins }, () => PGS_SIGNS),
  ];
  const english = { track: tracks[1], url: ENGLISH, format: SubtitleFormat.WEBVTT };
  const signs = tracks.slice(2).map((track, turn) => ({
    track,
    url: `/media/versions/7/subtitles/${turn + 2}`,
    format: SubtitleFormat.PGS,
  }));
  const burnInTracks = signs.map((_, turn) => turn + 2);
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
        const draws = deviceDecoding?.subtitleFormats.includes(SubtitleFormat.PGS) ?? false;
        const plan =
          subtitleTrack === undefined
            ? { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" }
            : { method: PlaybackMethod.TRANSCODE, url: "/media/plans/1/index.m3u8" };
        return draws
          ? { plan: { ...plan, subtitles: [english, ...signs] } }
          : { plan: { ...plan, subtitles: [english], burnInTracks, subtitleTrack } };
      },
    });
  });
}

/**
 * Opens the player on [`server`] with `twins`, its subtitles fetched as `fetched` answers, as
 * `asked` records its plans.
 */
async function playing(
  asked: PlanPlaybackRequest[],
  fetched = () => Promise.resolve(new Response("PGS")),
  twins = 0,
) {
  vi.spyOn(globalThis, "fetch").mockImplementation(fetched);
  openApp(PLAYER, server(asked, twins));
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  return video;
}

function choose(name: RegExp) {
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  fireEvent.click(screen.getByRole("menuitemradio", { name }));
}

test("this Device says it draws PGS, so the PGS Track is a subtitle the player shows itself", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const video = await playing(asked);

  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));

  const items = screen.getAllByRole("menuitemradio").map((item) => item.textContent);
  expect(items).toEqual(["Off", "English", "English Signs"]);
  expect(asked[0]?.deviceDecoding?.subtitleFormats).toEqual([SubtitleFormat.PGS]);
  // Only the text one is a `<track>`: the PGS one is drawn.
  const tracks = [...video.querySelectorAll("track")].map((track) => track.getAttribute("src"));
  expect(tracks).toEqual([ENGLISH]);
});

test("turning a PGS Track on draws it once fetched, with no new plan", async () => {
  const asked: PlanPlaybackRequest[] = [];
  let answer: ((response: Response) => void) | undefined;
  const fetched = new Promise<Response>((resolve) => {
    answer = resolve;
  });
  const video = await playing(asked, () => fetched);

  choose(/Signs/u);

  const signs = screen.getByRole("menuitemradio", { name: /Signs/u });
  await waitFor(() => {
    expect(signs.textContent).toContain("Loading…");
  });
  answer?.(new Response("PGS"));
  await waitFor(() => {
    expect(signs.textContent).not.toContain("Loading…");
  });
  expect(bitsub.drawn).toEqual(["PGS"]);
  expect(vi.mocked(fetch)).toHaveBeenCalledWith(SIGNS);
  expect(signs.getAttribute("aria-checked")).toBe("true");
  expect(asked).toHaveLength(1);
  expect(screen.getByLabelText("Alien")).toBe(video);
});

test("switching between a PGS Track, a text one and Off doesn't restart playback", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const video = await playing(asked);
  choose(/Signs/u);
  await waitFor(() => {
    expect(bitsub.drawn).toEqual(["PGS"]);
  });

  fireEvent.click(screen.getByRole("menuitemradio", { name: /^English$/u }));
  await waitFor(() => {
    expect(bitsub.disposed).toBe(1);
  });
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Signs/u }));
  await waitFor(() => {
    expect(bitsub.drawn).toEqual(["PGS", "PGS"]);
  });
  fireEvent.click(screen.getByRole("menuitemradio", { name: "Off" }));

  await waitFor(() => {
    expect(bitsub.disposed).toBe(2);
  });
  expect(asked).toHaveLength(1);
  expect(screen.getByLabelText("Alien")).toBe(video);
});

test("a PGS Track that can't be fetched says so, as a text one does", async () => {
  await playing([], () => Promise.resolve(new Response("", { status: 500 })));

  choose(/Signs/u);

  const signs = screen.getByRole("menuitemradio", { name: /Signs/u });
  await waitFor(() => {
    expect(signs.textContent).toContain("Couldn’t load");
  });
  expect(bitsub.drawn).toEqual([]);
});

// Last: once the renderer fails, this page never lists PGS again, as after it a reload would.
test("a renderer that won't start has the Track burned in by a plan that doesn't list PGS", async () => {
  const asked: PlanPlaybackRequest[] = [];
  bitsub.failsToStart = true;
  // Two PGS Tracks alike, so the second must be told from the first by its turn.
  const directPlay = await playing(asked, undefined, 1);

  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  for (const second of screen.getAllByRole("menuitemradio", { name: /Signs/u }).slice(1)) {
    fireEvent.click(second);
  }

  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked.map((request) => request.subtitleTrack)).toEqual([undefined, 3]);
  expect(asked[1]?.deviceDecoding?.subtitleFormats).toEqual([]);
  await waitFor(() => {
    expect(screen.getByLabelText("Alien")).not.toBe(directPlay);
  });
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  const checked = screen
    .getAllByRole("menuitemradio")
    .map((item) => item.getAttribute("aria-checked"));
  expect(checked).toEqual(["false", "false", "false", "true"]);
});
