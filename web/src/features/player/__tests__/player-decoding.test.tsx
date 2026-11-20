import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  AudioCodec,
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
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
    // oxlint-disable-next-line require-await -- a fake answers at once
    const decodingInfo = async (configuration: MediaDecodingConfiguration) => ({
      supported: JSON.stringify(configuration).includes(decoded),
    });
    Object.defineProperty(navigator, "mediaCapabilities", {
      value: { decodingInfo },
      configurable: true,
    });
    const asked: PlanPlaybackRequest[] = [];
    openApp("/libraries/1/movies/tmdb-603/play", serverAsked(asked));

    await screen.findByLabelText("The Matrix");

    expect(asked.map((request) => request.deviceDecoding?.h26410bit)).toStrictEqual([h26410bit]);
  },
);

/** What the fake `decodingInfo` refuses from each type: Vorbis files, and E-AC-3 and H.264 High 10
 * from MSE. */
const refusals: Record<string, string[]> = {
  file: ["vorbis"],
  "media-source": ["ec-3", "avc1.6E0028"],
};

/**
 * Lends the browser MSE and a `decodingInfo` that decodes all but `refusals`, which it rejects if
 * `rejects`, or else says are unsupported. Each type it's asked about goes in `types`.
 */
function lendMse(rejects: boolean, types: MediaDecodingType[] = []) {
  // The app only asks whether `MediaSource` exists.
  vi.stubGlobal("MediaSource", {});
  // oxlint-disable-next-line require-await -- a fake answers at once
  const decodingInfo = async (configuration: MediaDecodingConfiguration) => {
    types.push(configuration.type);
    const asked = JSON.stringify(configuration);
    if (!(refusals[configuration.type] ?? []).some((contentType) => asked.includes(contentType))) {
      return { supported: true };
    }
    if (rejects) {
      throw new TypeError("unsupported");
    }
    return { supported: false };
  };
  Object.defineProperty(navigator, "mediaCapabilities", {
    value: { decodingInfo },
    configurable: true,
  });
}

test.each([
  { refusal: "says unsupported", rejects: false },
  { refusal: "rejects", rejects: true },
])(
  "without native HLS, a codec is left out unless files and MSE both decode it, when MSE $refusal",
  async ({ rejects }) => {
    lendMse(rejects);
    const asked: PlanPlaybackRequest[] = [];
    openApp("/libraries/1/movies/tmdb-603/play", serverAsked(asked));

    await screen.findByLabelText("The Matrix");

    const [decoding] = asked.map((request) => request.deviceDecoding);
    expect(decoding?.audioCodecs).toContain(AudioCodec.AAC);
    expect(decoding?.audioCodecs).not.toContain(AudioCodec.EAC3);
    expect(decoding?.audioCodecs).not.toContain(AudioCodec.VORBIS);
    expect(decoding?.h26410bit).toBe(false);
  },
);

test("with native HLS, only files are asked about and MSE's refusals don't count", async () => {
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockReturnValue("maybe");
  const types: MediaDecodingType[] = [];
  lendMse(false, types);
  const asked: PlanPlaybackRequest[] = [];
  openApp("/libraries/1/movies/tmdb-603/play", serverAsked(asked));

  await screen.findByLabelText("The Matrix");

  const [decoding] = asked.map((request) => request.deviceDecoding);
  expect(decoding?.audioCodecs).toContain(AudioCodec.EAC3);
  expect(decoding?.h26410bit).toBe(true);
  expect(new Set(types)).toStrictEqual(new Set(["file"]));
});
