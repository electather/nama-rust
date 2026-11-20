import { Code, ConnectError } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { type ReportProgressRequest, WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const DURATION_S = 7000;

/** A fake Server with Alien in Library 1, which remembers the last report, failing the first `failures`. */
function server(failures = 0) {
  const reports: ReportProgressRequest[] = [];
  let failed = 0;
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { namespace: "tmdb", value: "348" },
          title: "Alien",
          versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n }],
          watchState: { positionMs: reports.at(-1)?.positionMs ?? 0n, durationMs: 7_000_000n },
        },
      }),
    });
    router.service(PlaybackService, {
      planPlayback: () => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" },
      }),
    });
    router.service(WatchStateService, {
      reportProgress: (request) => {
        if (failed < failures) {
          failed += 1;
          throw new ConnectError("Server unreachable", Code.Unavailable);
        }
        reports.push(request);
        return {};
      },
    });
  });
  return { transport, reports };
}

/** Finds the player's `<video>` once its file's length is known, as the browser would load it. */
async function loadedVideo() {
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "duration", { value: DURATION_S });
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

test("a report that fails is sent again, and nothing is shown", async () => {
  const { transport, reports } = server(1);
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
 * A fake Server with Episode 3 of Dark's Season 1 in Library 2, played once a report reaches its
 * end, which counts how often each browse call is asked.
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

test("playing an Episode only reports, and leaving it shows it played", async () => {
  const { transport, reports, asked } = episodeServer();
  const router = openApp("/libraries/2/shows/tmdb-70523/seasons/1/episodes/3/play", transport);
  const video = await screen.findByLabelText<HTMLVideoElement>("Past and Present");
  Object.defineProperty(video, "duration", { value: 3000 });
  fireEvent(video, new Event("loadedmetadata"));
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
