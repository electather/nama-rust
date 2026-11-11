import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  PickedTrackKind,
  type PlanPlaybackRequest,
  PlaybackMethod,
  PlaybackService,
  type ReportTrackPickRequest,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

vi.mock("hls.js", () => ({
  default: class {
    static Events = { ERROR: "hlsError" };
    on() {}
    loadSource() {}
    attachMedia() {}
    destroy() {}
  },
}));

// happy-dom doesn't name the ready states the player reads.
Object.assign(HTMLMediaElement, { HAVE_METADATA: 1 });

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";

const audio = (channels: number) => ({ case: "audio", value: { channels } }) as const;

/**
 * A fake Server with Alien in Library 1, whose one Version holds English 5.1 audio, played by
 * default, a stereo commentary and a subtitle, offering the audio Tracks `audioTracks`, both
 * unless given. It Direct Plays the English and Remuxes any other audio Track, each plan request
 * goes in `asked`, and each pick reported in `picked`.
 */
function server(
  asked: PlanPlaybackRequest[],
  audioTracks = [2, 3],
  picked: ReportTrackPickRequest[] = [],
) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const tracks = [
    { codec: "h264", kind: { case: "video", value: {} } },
    { codec: "subrip", language: "eng", kind: { case: "subtitle", value: {} } },
    { codec: "dts", language: "eng", default: true, kind: audio(6) },
    { codec: "aac", language: "eng", title: "Commentary", kind: audio(2) },
  ] as const;
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
        const audioTrack = request.audioTrack ?? 2;
        const played =
          audioTrack === 2
            ? { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" }
            : { method: PlaybackMethod.REMUX, url: `/media/plans/${asked.length}/index.m3u8` };
        return { plan: { ...played, audioTrack, audioTracks } };
      },
      reportTrackPick: (request) => {
        picked.push(request);
        return {};
      },
    });
  });
}

function openAudio() {
  fireEvent.click(screen.getByRole("button", { name: "Audio" }));
  return screen.getAllByRole("menuitemradio");
}

test("the audio menu lists the Version's audio Tracks by language, title and channels, the file's own chosen", async () => {
  openApp(PLAYER, server([]));
  await screen.findByLabelText("Alien");

  const items = openAudio();

  expect(items.map((item) => item.textContent)).toEqual([
    "English 5.1",
    "English Commentary · Stereo",
  ]);
  expect(items.map((item) => item.getAttribute("aria-checked"))).toEqual(["true", "false"]);
});

test("a Server that can only play the file’s own audio has no audio menu", async () => {
  openApp(PLAYER, server([], [2]));
  await screen.findByLabelText("Alien");

  expect(screen.queryByRole("button", { name: "Audio" })).toBeNull();
});

test("choosing the commentary mid-play asks for a plan from where the video stands with that audio Track", async () => {
  const asked: PlanPlaybackRequest[] = [];
  openApp(PLAYER, server(asked));
  const english = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(english, "readyState", { value: HTMLMediaElement.HAVE_METADATA });
  english.currentTime = 754.25;

  openAudio();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Commentary/u }));

  await waitFor(() => {
    expect(asked).toHaveLength(2);
  });
  expect(asked.map((request) => request.audioTrack)).toEqual([undefined, 3]);
  expect(asked[1]?.startPositionMs).toBe(754_250n);
  const commentary = await waitFor(() => {
    const video = screen.getByLabelText<HTMLVideoElement>("Alien");
    expect(video).not.toBe(english);
    return video;
  });
  // As in a browser, where a new source loads from its start; happy-dom keeps the old time.
  commentary.currentTime = 0;
  fireEvent(commentary, new Event("loadedmetadata"));
  expect(commentary.currentTime).toBe(754.25);
  expect(openAudio().map((item) => item.getAttribute("aria-checked"))).toEqual(["false", "true"]);
});

test("choosing an audio Track reports it as picked, for the Server to keep for the Movie", async () => {
  const picked: ReportTrackPickRequest[] = [];
  openApp(PLAYER, server([], [2, 3], picked));
  await screen.findByLabelText("Alien");

  openAudio();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Commentary/u }));

  await waitFor(() => {
    expect(picked.map(({ versionId, kind, track }) => ({ versionId, kind, track }))).toEqual([
      { versionId: 7n, kind: PickedTrackKind.AUDIO, track: 3 },
    ]);
  });
});
