import { cleanup, screen } from "@testing-library/react";
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
  // happy-dom has no `navigator.mediaCapabilities`; each test lends it one.
  Reflect.deleteProperty(navigator, "mediaCapabilities");
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
  { decoded: "avc1.640028", h26410bit: false },
  { decoded: "avc1.6E0028", h26410bit: true },
])(
  "the plan request says h26410bit is $h26410bit when decodingInfo accepts only $decoded",
  async ({ decoded, h26410bit }) => {
    const decodingInfo = (configuration: MediaDecodingConfiguration) =>
      Promise.resolve({ supported: JSON.stringify(configuration).includes(decoded) });
    Object.defineProperty(navigator, "mediaCapabilities", {
      value: { decodingInfo },
      configurable: true,
    });
    const asked: PlanPlaybackRequest[] = [];
    openApp("/libraries/1/movies/tmdb-603/play", serverAsked(asked));

    await screen.findByLabelText("The Matrix");

    expect(asked.map((request) => request.deviceDecoding?.h26410bit)).toEqual([h26410bit]);
  },
);
