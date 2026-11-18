import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

/** What the player set on the browser's Media Session, which happy-dom lacks. */
const session: {
  metadata: MediaMetadataInit | null;
  handlers: Map<MediaSessionAction, MediaSessionActionHandler>;
  position: MediaPositionState | undefined;
  setActionHandler: (action: MediaSessionAction, handler: MediaSessionActionHandler | null) => void;
  setPositionState: (state?: MediaPositionState) => void;
} = {
  metadata: null,
  handlers: new Map(),
  position: undefined,
  setActionHandler(action: MediaSessionAction, handler: MediaSessionActionHandler | null) {
    if (handler) {
      this.handlers.set(action, handler);
    } else {
      this.handlers.delete(action);
    }
  },
  setPositionState(state?: MediaPositionState) {
    this.position = state;
  },
};

beforeEach(() => {
  session.metadata = null;
  session.handlers.clear();
  session.position = undefined;
  Object.defineProperty(navigator, "mediaSession", { value: session, configurable: true });
  vi.stubGlobal(
    "MediaMetadata",
    vi.fn(function metadata(this: MediaMetadataInit, init: MediaMetadataInit) {
      Object.assign(this, init);
    }),
  );
});

afterEach(() => {
  cleanup();
  Reflect.deleteProperty(navigator, "mediaSession");
  vi.unstubAllGlobals();
});

const server = fakeServer(
  {
    signedIn: newUser,
    libraries: [
      { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
      { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
    ],
  },
  {},
  {},
  (router) => {
    router.service(PlaybackService, {
      planPlayback: ({ versionId }) => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: `/media/versions/${versionId}` },
      }),
    });
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { namespace: "tmdb", value: "603" },
          title: "The Matrix",
          posterUrl: "/images/matrix.jpg",
          versions: [{ id: 7n, fileName: "The Matrix.mkv", size: 1n }],
        },
      }),
      getShow: () => ({
        show: {
          id: { namespace: "tmdb", value: "70523" },
          title: "Dark",
          posterUrl: "/images/dark.jpg",
        },
      }),
      listEpisodes: () => ({
        episodes: [
          {
            id: { namespace: "tmdb", value: "3" },
            seasonNumber: 1,
            episodeNumber: 3,
            title: "Past and Present",
            versions: [{ id: 3n, fileName: "Dark S01E03.mkv", size: 1n }],
          },
        ],
      }),
      getNextEpisode: () => ({}),
    });
  },
);

/** Presses a lock-screen button or media key, as the browser hands it to the player. */
function press(name: MediaSessionAction, details: Partial<MediaSessionActionDetails> = {}) {
  const handler = session.handlers.get(name);
  if (!handler) {
    throw new Error(`no ${name} handler`);
  }
  handler({ action: name, ...details });
}

test("the lock screen shows a Movie by its title and Poster", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  await screen.findByLabelText("The Matrix");

  expect(session.metadata).toMatchObject({
    title: "The Matrix",
    artist: "",
    artwork: [{ src: "/images/matrix.jpg" }],
  });
});

test("the lock screen shows an Episode with its Show and which Episode it is, by the Show's Poster", async () => {
  openApp("/libraries/2/shows/tmdb-70523/seasons/1/episodes/3/play", server);
  await screen.findByLabelText("Past and Present");

  await waitFor(() => {
    expect(session.metadata).toMatchObject({
      title: "Past and Present",
      artist: "Dark · S1, E3",
      artwork: [{ src: "/images/dark.jpg" }],
    });
  });
});

test("media keys play, pause and seek by the player's step, and the lock screen follows the position", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  Object.defineProperty(video, "duration", { value: 7000 });
  video.currentTime = 100;
  fireEvent(video, new Event("durationchange"));
  expect(session.position).toStrictEqual({ duration: 7000, position: 100, playbackRate: 1 });

  press("pause");
  expect(video.paused).toBe(true);
  press("play");
  expect(video.paused).toBe(false);
  press("seekforward");
  expect(video.currentTime).toBe(110);
  press("seekbackward", { seekOffset: 30 });
  expect(video.currentTime).toBe(80);
  press("seekto", { seekTime: 5 });
  expect(video.currentTime).toBe(5);
  press("seekbackward");
  expect(video.currentTime).toBe(0);
  fireEvent(video, new Event("timeupdate"));
  expect(session.position).toStrictEqual({ duration: 7000, position: 0, playbackRate: 1 });
});

test("before the video knows its length, the lock screen shows no position", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  Object.defineProperty(video, "duration", { value: Number.NaN });

  fireEvent(video, new Event("timeupdate"));

  expect(session.position).toBeUndefined();
});

test("leaving the player clears the lock screen and the media keys", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  Object.defineProperty(video, "duration", { value: 7000 });
  fireEvent(video, new Event("durationchange"));

  cleanup();

  expect(session.metadata).toBeNull();
  expect(session.handlers.size).toBe(0);
  expect(session.position).toBeUndefined();
});
