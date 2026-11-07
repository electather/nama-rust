import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { TransportProvider, useSuspenseQuery } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Suspense, useRef } from "react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { type ReportProgressRequest, WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { useWatchState } from "@/lib/watch-state";

afterEach(cleanup);

const movie = { libraryId: 1n, movieId: { id: { case: "tmdb", value: 348n } } } as const;
const DURATION_S = 7000;

/** A fake Server that remembers the last report, failing the first `failures` of them. */
function server(failures = 0) {
  const reports: ReportProgressRequest[] = [];
  let failed = 0;
  const transport = createRouterTransport((router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          title: "Alien",
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

/** What the player (#72) does with a Movie: plays it on a `<video>` and keeps its Watch State. */
function Player() {
  const { data } = useSuspenseQuery(BrowseService.method.getMovie, movie);
  const ref = useRef<HTMLVideoElement>(null);
  useWatchState(ref, 7n, data.movie?.watchState);
  // oxlint-disable-next-line jsx-a11y/media-has-caption -- A stand-in for the player.
  return <video ref={ref} aria-label="Alien" />;
}

function openPlayer(transport: ReturnType<typeof createRouterTransport>, queryClient: QueryClient) {
  render(
    <TransportProvider transport={transport}>
      <QueryClientProvider client={queryClient}>
        <Suspense>
          <Player />
        </Suspense>
      </QueryClientProvider>
    </TransportProvider>,
  );
}

/** Finds the `<video>` once its file's length is known, as the browser would load it. */
async function loadedVideo() {
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  Object.defineProperty(video, "duration", { value: DURATION_S });
  fireEvent(video, new Event("loadedmetadata"));
  return video;
}

test("reopening a Movie resumes where it stopped", async () => {
  const { transport, reports } = server();
  const queryClient = new QueryClient();
  openPlayer(transport, queryClient);
  const video = await loadedVideo();
  video.currentTime = 600;
  // Leaving reports where it stopped.
  cleanup();
  await waitFor(() => {
    expect(reports.at(-1)?.positionMs).toBe(600_000n);
  });

  openPlayer(transport, queryClient);

  expect((await loadedVideo()).currentTime).toBe(600);
});

test("a report that fails is sent again, and nothing is shown", async () => {
  const { transport, reports } = server(1);
  openPlayer(transport, new QueryClient());
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
  expect(document.body.textContent).toBe("");
});

test("hiding the page reports where it is", async () => {
  const { transport, reports } = server();
  openPlayer(transport, new QueryClient());
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
