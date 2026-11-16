import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  AudioCodec,
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
  VideoCodec,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(() => {
  cleanup();
  // happy-dom has no `navigator.mediaCapabilities`; each test lends it one.
  Reflect.deleteProperty(navigator, "mediaCapabilities");
  vi.unstubAllGlobals();
});

/** A Server whose Library 1 has The Matrix, Direct Played, putting each plan request in `asked`. */
const serverAsked = (asked: PlanPlaybackRequest[]) =>
  fakeServer(
    { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
    {},
    {},
    (router) => {
      router.service(PlaybackService, {
        planPlayback: (request) => {
          asked.push(request);
          return { plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" } };
        },
      });
      router.service(BrowseService, {
        getMovie: () => ({
          movie: {
            id: { namespace: "tmdb", value: "603" },
            title: "The Matrix",
            versions: [{ id: 7n, fileName: "The Matrix (1999).mkv", size: 1n }],
          },
        }),
      });
    },
  );

test.each([
  { decoded: ["avc1.640028", "mp4a.40.2"], h26410bit: false },
  { decoded: ["avc1.640028", "avc1.6E0028", "mp4a.40.2"], h26410bit: true },
])("the plan request says what this Device decodes of $decoded", async ({ decoded, h26410bit }) => {
  const decodingInfo = (configuration: MediaDecodingConfiguration) => {
    const json = JSON.stringify(configuration);
    return Promise.resolve({ supported: decoded.some((codec) => json.includes(codec)) });
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
  openApp("/libraries/1/movies/tmdb-603/play", serverAsked(asked));

  await screen.findByLabelText("The Matrix");

  expect(asked.map((request) => request.deviceDecoding)).toMatchObject([
    { videoCodecs: [VideoCodec.H264], audioCodecs: [AudioCodec.AAC], audioChannels: 6, h26410bit },
  ]);
});
