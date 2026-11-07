import { Code, ConnectError } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { Role } from "@/gen/nama/v1/sign_in_pb";
import { type ReportProgressRequest, WatchStateService } from "@/gen/nama/v1/watch_state_pb";

import { fakeServer, openApp } from "./test-app";

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const DURATION_S = 7000;

/** A fake Server with Alien in Library 1, which remembers the last report, failing the first `failures`. */
function server(failures = 0) {
  const reports: ReportProgressRequest[] = [];
  let failed = 0;
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer({ signedIn: Role.MEMBER, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 348n } },
          title: "Alien",
          versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n }],
          watchState: { positionMs: reports.at(-1)?.positionMs ?? 0n, durationMs: 7_000_000n },
        },
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
  expect(await screen.findByRole("progressbar", { name: "Watched" })).toBeDefined();
  expect(reports.at(-1)?.positionMs).toBe(600_000n);
  fireEvent.click(screen.getByRole("link", { name: "Play" }));

  expect((await loadedVideo()).currentTime).toBe(600);
});

test("a report that fails is sent again, and nothing is shown", async () => {
  const { transport, reports } = server(1);
  openApp(PLAYER, transport);
  const video = await loadedVideo();
  video.currentTime = 42;
  fireEvent(video, new Event("pause"));

  await waitFor(
    () => {
      expect(reports.map(({ positionMs }) => positionMs)).toEqual([42_000n]);
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
  // oxlint-disable-next-line typescript/no-dynamic-delete -- Back to the document's own getter.
  delete (document as { visibilityState?: unknown }).visibilityState;

  await waitFor(() => {
    expect(reports.map(({ positionMs }) => positionMs)).toEqual([90_000n]);
  });
});
