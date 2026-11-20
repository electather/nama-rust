import { Code, ConnectError } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { type ReportProgressRequest, WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const DURATION_S = 7000;

/**
 * A fake Server with Alien in Library 1, which remembers the last report, failing the first
 * `failures`, and if `held` answers those sent so far only once `landed` is called. `moviesAsked`
 * counts `GetMovie`.
 */
function server({ failures = 0, held = false } = {}) {
  const reports: ReportProgressRequest[] = [];
  let failed = 0;
  let moviesAsked = 0;
  const holding: (() => void)[] = [];
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => {
        moviesAsked += 1;
        return {
          movie: {
            id: { namespace: "tmdb", value: "348" },
            title: "Alien",
            versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n }],
            watchState: { positionMs: reports.at(-1)?.positionMs ?? 0n, durationMs: 7_000_000n },
          },
        };
      },
    });
    router.service(PlaybackService, {
      planPlayback: () => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" },
      }),
    });
    router.service(WatchStateService, {
      reportProgress: async (request) => {
        if (held) {
          await new Promise<void>((resolve) => {
            holding.push(resolve);
          });
        }
        if (failed < failures) {
          failed += 1;
          throw new ConnectError("Server unreachable", Code.Unavailable);
        }
        reports.push(request);
        return {};
      },
    });
  });
  const landed = () => {
    for (const land of holding) {
      land();
    }
  };
  return { transport, reports, landed, moviesAsked: () => moviesAsked };
}

/** Finds the player's `<video>` once its file's length is known, as the browser would load it. */
async function loadedVideo(title = "Alien", durationS = DURATION_S) {
  const video = await screen.findByLabelText<HTMLVideoElement>(title);
  Object.defineProperty(video, "duration", { value: durationS });
  fireEvent(video, new Event("loadedmetadata"));
  return video;
}

test("reopening a Movie resumes where it stopped", async () => {
  const { transport, reports } = server();
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  video.currentTime = 600;

  // Leaving reports where it stopped, and the Movie's page shows how far in it is.
  fireEvent.click(screen.getByRole("link", { name: "Back" }));
  const resume = await screen.findByRole("link", { name: /^Resume/u });
  expect(reports.at(-1)?.positionMs).toBe(600_000n);
  fireEvent.click(resume);

  const resumed = await loadedVideo();
  expect(resumed.currentTime).toBe(600);
});

test("reports while a Movie plays ask for no Movie again", async () => {
  const { transport, reports, moviesAsked } = server();
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  const asked = moviesAsked();

  // The first `timeupdate` is due a report at once; then a pause and a seek.
  for (const event of ["timeupdate", "pause", "seeked"]) {
    fireEvent(video, new Event(event));
  }

  await waitFor(() => {
    expect(reports).toHaveLength(3);
  });
  await act(async () => {});
  expect(moviesAsked()).toBe(asked);
});

test("leaving after playing a while asks for the Movie once", async () => {
  const { transport, moviesAsked } = server();
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  video.currentTime = 600;
  const asked = moviesAsked();
  // Past how long the Movie's page reuses what the player read.
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(Date.now() + 60_000);

  fireEvent.click(screen.getByRole("link", { name: "Back" }));
  await screen.findByRole("link", { name: /^Resume/u });
  await act(async () => {});
  expect(moviesAsked()).toBe(asked + 1);
});

test("the report on leaving refreshes the Movie's page even when it lands after the page opens", async () => {
  const { transport, landed } = server({ held: true });
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  video.currentTime = 600;

  fireEvent.click(screen.getByRole("link", { name: "Back" }));
  await screen.findByRole("heading", { name: "Alien" });
  expect(screen.queryByRole("link", { name: /^Resume/u })).toBeNull();
  landed();

  await expect(screen.findByRole("link", { name: /^Resume/u })).resolves.toBeDefined();
});

test("a report that fails is sent again, and nothing is shown", async () => {
  const { transport, reports } = server({ failures: 1 });
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  video.currentTime = 42;
  fireEvent(video, new Event("pause"));

  await waitFor(
    () => {
      expect(reports.map(({ positionMs }) => positionMs)).toStrictEqual([42_000n]);
    },
    { timeout: 3000 },
  );
  await act(async () => {});
  expect(screen.getByLabelText("Alien")).toBe(video);
  expect(screen.queryByRole("alert")).toBeNull();
});

test("hiding the page reports where it is", async () => {
  const { transport, reports } = server();
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  video.currentTime = 90;
  Object.defineProperty(document, "visibilityState", { value: "hidden", configurable: true });
  fireEvent(document, new Event("visibilitychange"));
  // Back to the document's own getter.
  Reflect.deleteProperty(document, "visibilityState");

  await waitFor(() => {
    expect(reports.map(({ positionMs }) => positionMs)).toStrictEqual([90_000n]);
  });
});

/**
 * A fake Server with Episode 3 of Dark's Season 1 in Library 2. It counts how often each browse
 * call is asked, and the Episode is played once a report reaches its end.
 */
function episodeServer() {
  const reports: ReportProgressRequest[] = [];
  const asked = { getShow: 0, listEpisodes: 0, getNextEpisode: 0 };
  const episode = () => ({
    id: { namespace: "tmdb", value: "3" },
    seasonNumber: 1,
    episodeNumber: 3,
    title: "Past and Present",
    versions: [{ id: 3n, fileName: "Dark S01E03.mkv", size: 1n }],
    watchState: { played: reports.at(-1)?.positionMs === 3_000_000n },
  });
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: () => {
        asked.getShow += 1;
        return {
          show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" },
          seasons: [{ number: 1 }],
        };
      },
      listEpisodes: () => {
        asked.listEpisodes += 1;
        return { episodes: [episode()] };
      },
      getNextEpisode: () => {
        asked.getNextEpisode += 1;
        return {};
      },
    });
    router.service(PlaybackService, {
      getAutoplay: () => ({ on: false }),
      planPlayback: () => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/3" },
      }),
    });
    router.service(WatchStateService, {
      reportProgress: (request) => {
        reports.push(request);
        return {};
      },
    });
  });
  return { transport, reports, asked };
}

test("reports while an Episode plays refetch no browsing, and leaving shows it played", async () => {
  const { transport, reports, asked } = episodeServer();
  const router = openApp("/libraries/2/shows/tmdb-70523/seasons/1/episodes/3/play", transport);
  const video = await loadedVideo("Past and Present", 3000);
  await waitFor(() => {
    expect(router.state.status).toBe("idle");
  });
  const opened = { ...asked };

  video.currentTime = 600;
  fireEvent(video, new Event("timeupdate"));
  fireEvent(video, new Event("pause"));
  fireEvent(video, new Event("seeked"));
  await waitFor(() => {
    expect(reports).toHaveLength(3);
  });
  await act(async () => {});
  expect(asked).toStrictEqual(opened);

  video.currentTime = 3000;
  fireEvent.click(screen.getByRole("link", { name: "Back" }));
  const watched = await screen.findByRole("button", { name: "Watched: Past and Present" });
  await waitFor(() => {
    expect(watched.getAttribute("aria-pressed")).toBe("true");
  });
});
