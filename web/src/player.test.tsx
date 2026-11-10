import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type TrackSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(() => {
  cleanup();
  // happy-dom has no `navigator.mediaCapabilities`; a test may lend it one.
  Reflect.deleteProperty(navigator, "mediaCapabilities");
});

const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
const cantDecode = "This Device can't decode its HEVC video, so it can't be Direct Played";
const plannedTracks: MessageInitShape<typeof TrackSchema>[] = [
  { codec: "h264", kind: { case: "video", value: { width: 1920, height: 1080 } } },
  { codec: "aac", language: "eng", kind: { case: "audio", value: { channels: 6 } } },
  { codec: "subrip", language: "fre", kind: { case: "subtitle", value: {} } },
];
/**
 * A Server whose Library 1 has The Matrix in three Versions, the largest being Version 8, and
 * whose plans Direct Play each from an address of its own with `plannedTracks`, or, unless
 * `playable`, say why it can't play. Each plan request goes in `asked`.
 */
const serverPlanning = (playable = true, asked: PlanPlaybackRequest[] = []) =>
  fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(PlaybackService, {
      planPlayback: (request) => {
        asked.push(request);
        return {
          plan: playable
            ? {
                method: PlaybackMethod.DIRECT_PLAY,
                url: `/planned/${request.versionId}`,
                tracks: plannedTracks,
                reasons: ["Its Tracks need no change"],
              }
            : { reasons: ["Its Tracks need no change", cantDecode] },
        };
      },
    });
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 603n } },
          title: "The Matrix",
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

test("a file the browser can't play shows a message instead of the player", async () => {
  openApp("/libraries/1/movies/tmdb-603/play?version=7", server);

  fireEvent.error(await findVideo());

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});

test("a Version whose plan says it can't play shows the plan's reasons instead of the player", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning(false));

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.getByText(cantDecode)).toBeDefined();
  expect(screen.queryByText(/This browser can’t play it/u)).toBeNull();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});

test("the plan request says what this Device decodes", async () => {
  const decodable = ['video/mp4; codecs="avc1.640028"', 'audio/mp4; codecs="mp4a.40.2"'];
  // A configuration names its content type, which its JSON holds as the type's own JSON.
  const decodingInfo = (configuration: MediaDecodingConfiguration) =>
    Promise.resolve({
      supported: decodable.some((type) =>
        JSON.stringify(configuration).includes(JSON.stringify(type)),
      ),
    });
  Object.defineProperty(navigator, "mediaCapabilities", {
    value: { decodingInfo },
    configurable: true,
  });
  const asked: PlanPlaybackRequest[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", serverPlanning(true, asked));

  await findVideo();

  expect(asked.map((request) => request.deviceDecoding?.contentTypes)).toEqual([decodable]);
});

test("an address with no title to play keeps the app chrome, so there's a way out", async () => {
  openApp("/libraries/2/movies/tmdb-603/play", server);

  expect(await screen.findByText("Page not found")).toBeDefined();
  expect(screen.getByRole("navigation", { name: "Main" })).toBeDefined();
});

test("playback info says how the plan plays the Version, why, and with which Tracks", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);

  const info = await screen.findByRole("button", { name: "Playback info" });
  expect(info.getAttribute("aria-expanded")).toBe("false");
  // A button, so Enter and Space open it from the keyboard as a click does.
  fireEvent.click(info);

  expect(info.getAttribute("aria-expanded")).toBe("true");
  const panel = within(screen.getByRole("region", { name: "Playback info" }));
  expect(panel.getByText("Direct Play")).toBeDefined();
  expect(panel.getByText("Its Tracks need no change")).toBeDefined();
  expect(panel.getByText("H.264 · 1080p")).toBeDefined();
  expect(panel.getByText("AAC · 5.1 · English")).toBeDefined();
  expect(panel.getByText("SRT · French")).toBeDefined();
});

test("Esc closes playback info and stays in the player, wherever focus is", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  fireEvent.click(await screen.findByRole("button", { name: "Playback info" }));

  // As after clicking the video, which takes no focus.
  fireEvent.keyDown(document.body, { key: "Escape" });

  expect(screen.queryByRole("region", { name: "Playback info" })).toBeNull();
  expect(screen.getByLabelText("The Matrix")).toBeDefined();
});
