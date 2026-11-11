import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService, type TrackSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  AudioCodec,
  type PlanPlaybackRequest,
  PlaybackMethod,
  type PlaybackPlanSchema,
  PlaybackService,
  VideoCodec,
} from "@/gen/nama/v1/playback_pb";

import type { WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";

import { fakeServer, newUser, openApp } from "./test-app";

/** What each hls.js player was given: the playlist it loads and the `<video>` it plays in. */
const hlsJs = vi.hoisted(() => ({ played: [] as { url: string; video: HTMLMediaElement }[] }));
vi.mock("hls.js", () => ({
  default: class {
    static Events = { ERROR: "hlsError" };
    url = "";
    on() {}
    loadSource(url: string) {
      this.url = url;
    }
    attachMedia(video: HTMLMediaElement) {
      hlsJs.played.push({ url: this.url, video });
    }
    destroy() {}
  },
}));

// happy-dom doesn't name the ready states the player reads.
Object.assign(HTMLMediaElement, { HAVE_METADATA: 1 });

afterEach(() => {
  cleanup();
  hlsJs.played = [];
  vi.restoreAllMocks();
  // happy-dom has no `navigator.mediaCapabilities`; a test may lend it one.
  Reflect.deleteProperty(navigator, "mediaCapabilities");
  vi.unstubAllGlobals();
});

const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
const cantDecode = "This Device can't decode its HEVC video, so it can't be Direct Played";
const plannedTracks: MessageInitShape<typeof TrackSchema>[] = [
  { codec: "h264", kind: { case: "video", value: { width: 1920, height: 1080 } } },
  { codec: "aac", language: "eng", kind: { case: "audio", value: { channels: 6 } } },
  { codec: "subrip", language: "fre", kind: { case: "subtitle", value: {} } },
];
type Planned = (request: PlanPlaybackRequest) => MessageInitShape<typeof PlaybackPlanSchema>;
const directPlay: Planned = (request) => ({
  method: PlaybackMethod.DIRECT_PLAY,
  url: `/planned/${request.versionId}`,
  tracks: plannedTracks,
  reasons: ["Its Tracks need no change"],
});
const cantPlay: Planned = () => ({ reasons: ["Its Tracks need no change", cantDecode] });
const remux: Planned = () => ({
  method: PlaybackMethod.REMUX,
  url: "/media/plans/0/index.m3u8",
  tracks: plannedTracks,
  reasons: ["This Device can't decode its Matroska container, so it can't be Direct Played"],
});
const directPlayFailed = "Direct Play failed on this Device";
/**
 * A Server whose Library 1 has The Matrix in three Versions, the largest being Version 8, and
 * whose plans are `planned`'s, by default Direct Playing each from an address of its own with
 * `plannedTracks`. As the real Server does, a plan asked for after Direct Play failed says it can't
 * play, unless the Server `ignoresFailures`. Each plan request goes in `asked`. The Movie's Watch
 * State is `watchState`.
 */
const serverPlanning = ({
  planned = directPlay,
  asked = [] as PlanPlaybackRequest[],
  ignoresFailures = false,
  watchState,
}: {
  planned?: Planned;
  asked?: PlanPlaybackRequest[];
  ignoresFailures?: boolean;
  watchState?: MessageInitShape<typeof WatchStateSchema>;
} = {}) =>
  fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(PlaybackService, {
      planPlayback: (request) => {
        asked.push(request);
        const failed =
          !ignoresFailures && request.failedMethods.includes(PlaybackMethod.DIRECT_PLAY);
        if (failed) return { plan: { reasons: ["Its Tracks need no change", directPlayFailed] } };
        return { plan: planned(request) };
      },
    });
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 603n } },
          title: "The Matrix",
          watchState,
          versions: [
            { id: 7n, fileName: "The Matrix (1999) 720p.mkv", size: 2_000_000_000n },
            { id: 8n, fileName: "The Matrix (1999) 2160p.mkv", size: 40_000_000_000n },
            { id: 9n, fileName: "The Matrix (1999) 1080p.mkv", size: 9_000_000_000n },
          ],
        },
      }),
    });
  });
const server = serverPlanning();

function findVideo() {
  return screen.findByLabelText("The Matrix");
}

test("without a version, the player plays the largest from where its plan says", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);

  expect((await findVideo()).getAttribute("src")).toBe("/planned/8");
  expect(screen.queryByRole("navigation", { name: "Main" })).toBeNull();
});

// Base UI warns on a 0..0 range even when the slider is disabled, so an unknown duration stands in 0..1.
test("the Position slider is disabled until the video knows a finite duration, then seeks", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  const position = screen.getByLabelText<HTMLInputElement>("Position");
  expect(position.disabled).toBe(true);
  expect(position.max).toBe("1");

  Object.defineProperty(video, "duration", { value: Infinity, configurable: true });
  fireEvent(video, new Event("durationchange"));
  expect(position.disabled).toBe(true);

  Object.defineProperty(video, "duration", { value: 7000 });
  fireEvent(video, new Event("durationchange"));

  expect(position.disabled).toBe(false);
  expect(position.max).toBe("7000");
  fireEvent.keyDown(position, { key: "End" });
  expect(video.currentTime).toBe(7000);
});

test("a file the browser fails to load is planned again from where it started, without Direct Play, which says why it can't play", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const watchState = { positionMs: 600_000n, durationMs: 7_000_000n };
  openApp("/libraries/1/movies/tmdb-603/play?version=7", serverPlanning({ asked, watchState }));

  fireEvent.error(await findVideo());

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.getByText(directPlayFailed)).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
  expect(asked.map((request) => request.failedMethods)).toEqual([[], [PlaybackMethod.DIRECT_PLAY]]);
  expect(asked.map((request) => request.startPositionMs)).toEqual([600_000n, 600_000n]);
});

/** As the real Server plans a file in a container the Device decodes: Direct Play, then a Remux. */
const remuxOnceDirectPlayFails: Planned = (request) => {
  const failed = (method: PlaybackMethod) => request.failedMethods.includes(method);
  if (failed(PlaybackMethod.REMUX)) return { reasons: [directPlayFailed, "Remux failed too"] };
  if (failed(PlaybackMethod.DIRECT_PLAY)) return { ...remux(request), reasons: [directPlayFailed] };
  return directPlay(request);
};

test("a Direct Play that fails mid-film goes on as a Remux from there, until that fails too", async () => {
  const asked: PlanPlaybackRequest[] = [];
  const planned = remuxOnceDirectPlayFails;
  const fallingBack = serverPlanning({ planned, asked, ignoresFailures: true });
  openApp("/libraries/1/movies/tmdb-603/play?version=7", fallingBack);
  const directPlaying = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  Object.defineProperty(directPlaying, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  directPlaying.currentTime = 1234.5;

  fireEvent.error(directPlaying);
  // The same failure reported again, a little later, as hls.js and the `<video>` may.
  directPlaying.currentTime = 1240;
  fireEvent.error(directPlaying);

  await waitFor(() => {
    expect(hlsJs.played).toHaveLength(1);
  });
  expect(asked.map((request) => request.startPositionMs)).toEqual([0n, 1_234_500n]);
  const remuxing = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  // As in a browser, where a new source loads from its start; happy-dom keeps the old time.
  remuxing.currentTime = 0;
  fireEvent(remuxing, new Event("loadedmetadata"));
  expect(remuxing.currentTime).toBe(1234.5);
  fireEvent.click(screen.getByRole("button", { name: "Playback info" }));
  expect(screen.getByText(directPlayFailed)).toBeDefined();

  fireEvent.error(remuxing);

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.getByText("Remux failed too")).toBeDefined();
});

test("a method that fails again gives up instead of asking for plans in a loop", async () => {
  const asked: PlanPlaybackRequest[] = [];
  openApp(
    "/libraries/1/movies/tmdb-603/play?version=7",
    serverPlanning({ asked, ignoresFailures: true }),
  );
  fireEvent.error(await findVideo());
  // The player is hidden while it asks again, and back once the new plan says Direct Play again.
  await screen.findByRole("button", { name: "Playback info" });

  fireEvent.error(await findVideo());

  expect(await screen.findByText(/This browser can’t play it/u)).toBeDefined();
  expect(asked).toHaveLength(2);
});

test("a Version whose plan says it can't play shows the plan's reasons instead of the player", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning({ planned: cantPlay }));

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.getByText(cantDecode)).toBeDefined();
  expect(screen.queryByText(/This browser can’t play it/u)).toBeNull();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});

test("the plan request says what this Device decodes", async () => {
  const h264AndAac = ["avc1.640028", "mp4a.40.2"];
  const decodingInfo = (configuration: MediaDecodingConfiguration) => {
    const json = JSON.stringify(configuration);
    return Promise.resolve({ supported: h264AndAac.some((codec) => json.includes(codec)) });
  };
  Object.defineProperty(navigator, "mediaCapabilities", {
    value: { decodingInfo },
    configurable: true,
  });
  vi.stubGlobal(
    "AudioContext",
    vi.fn(function audioContext() {
      return { destination: { maxChannelCount: 6 }, close: () => Promise.resolve() };
    }),
  );
  const asked: PlanPlaybackRequest[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning({ asked }));

  await findVideo();

  expect(asked.map((request) => request.deviceDecoding)).toMatchObject([
    { videoCodecs: [VideoCodec.H264], audioCodecs: [AudioCodec.AAC], audioChannels: 6 },
  ]);
});

test("a Remux plays its HLS playlist through hls.js in a browser without HLS of its own", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning({ planned: remux }));

  const video = await findVideo();

  await waitFor(() => {
    expect(hlsJs.played).toEqual([{ url: "/media/plans/0/index.m3u8", video }]);
  });
  expect(video.getAttribute("src")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Playback info" }));
  const panel = within(screen.getByRole("region", { name: "Playback info" }));
  expect(panel.getByText("Remux")).toBeDefined();
  expect(
    panel.getByText(
      "This Device can't decode its Matroska container, so it can't be Direct Played",
    ),
  ).toBeDefined();
});

// The playlist lists the whole Version from its start, so a Remux's time is the file's.
test("a Remux resumes from the Watch State's point in the file", async () => {
  const watchState = { positionMs: 600_000n, durationMs: 7_000_000n };
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning({ planned: remux, watchState }));
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  await waitFor(() => {
    expect(hlsJs.played).toHaveLength(1);
  });

  Object.defineProperty(video, "duration", { value: 7000 });
  fireEvent(video, new Event("loadedmetadata"));

  expect(video.currentTime).toBe(600);
});

test("a Remux plays its HLS playlist natively in a browser with HLS of its own", async () => {
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockReturnValue("maybe");
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning({ planned: remux }));

  expect((await findVideo()).getAttribute("src")).toBe("/media/plans/0/index.m3u8");
  expect(hlsJs.played).toEqual([]);
});

test("an address with no title to play keeps the app chrome, so there's a way out", async () => {
  openApp("/libraries/2/movies/tmdb-603/play", server);

  expect(await screen.findByText("Page not found")).toBeDefined();
  expect(screen.getByRole("navigation", { name: "Main" })).toBeDefined();
});
