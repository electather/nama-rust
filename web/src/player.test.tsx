import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
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
/**
 * A Server whose Library 1 has The Matrix in three Versions, the largest being Version 8, and
 * whose plans Direct Play each from an address of its own, or, unless `playable`, say why it can't
 * play. Each plan request goes in `asked`.
 */
const serverPlanning = (playable = true, asked: PlanPlaybackRequest[] = []) =>
  fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(PlaybackService, {
      planPlayback: (request) => {
        asked.push(request);
        return {
          plan: playable
            ? { method: PlaybackMethod.DIRECT_PLAY, url: `/planned/${request.versionId}` }
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
