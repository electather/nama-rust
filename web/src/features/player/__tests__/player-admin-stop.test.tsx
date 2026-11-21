import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { eventFeed, fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-603/play";
const MESSAGE = "Restarting the Server for an upgrade";

/**
 * A Server whose Library 1 has The Matrix, each plan for it numbered from 1 in `plans`, and whose
 * Events `events` sends. `moviesAsked` counts `GetMovie`.
 */
function matrixServer() {
  const events = eventFeed();
  let plans = 0;
  let moviesAsked = 0;
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer(
    { signedIn: newUser, libraries, events: events.stream },
    {},
    {},
    (router) => {
      router.service(PlaybackService, {
        planPlayback: () => {
          plans += 1;
          const plan = { id: BigInt(plans), method: PlaybackMethod.DIRECT_PLAY, url: "/v/7" };
          return { plan };
        },
      });
      router.service(BrowseService, {
        getMovie: () => {
          moviesAsked += 1;
          const versions = [{ id: 7n, fileName: "Matrix.mkv", size: 1n }];
          return {
            movie: { id: { namespace: "tmdb", value: "603" }, title: "The Matrix", versions },
          };
        },
      });
    },
  );
  return { transport, events, plans: () => plans, moviesAsked: () => moviesAsked };
}

type Server = ReturnType<typeof matrixServer>;

/** Sends that the plan `planId` ended, stopped by an Admin who said `message` if one is given. */
function ended({ events }: Server, planId: bigint, message?: string) {
  const adminStop = message === undefined ? undefined : { message };
  events.send({ event: { case: "playbackEnded", value: { playback: { planId }, adminStop } } });
}

/** Waits until the app has applied every Event sent so far and rendered what they change. */
async function eventsApplied({ events, moviesAsked }: Server) {
  const asked = moviesAsked();
  // Events apply in order, so once this later one refetches the Movie, the earlier ones are in.
  events.send({ event: { case: "libraryItemRemoved", value: { libraryId: 1n } } });
  await waitFor(() => {
    expect(moviesAsked()).toBeGreaterThan(asked);
  });
  await act(async () => {});
}

test("an Admin's stop closes the player and shows what they said", async () => {
  const server = matrixServer();
  openApp(PLAYER, server.transport);
  await screen.findByLabelText("The Matrix");

  ended(server, 1n, MESSAGE);

  await expect(
    screen.findByRole("heading", { name: "An Admin stopped playback" }),
  ).resolves.toBeDefined();
  expect(screen.getByText(MESSAGE)).toBeDefined();
  expect(screen.queryByLabelText("The Matrix")).toBeNull();
});

test("another plan's stop, or this plan ending without an Admin, leaves the player open", async () => {
  const server = matrixServer();
  openApp(PLAYER, server.transport);
  await screen.findByLabelText("The Matrix");

  ended(server, 2n, MESSAGE);
  ended(server, 1n);
  await eventsApplied(server);

  expect(screen.queryByRole("heading", { name: "An Admin stopped playback" })).toBeNull();
  expect(screen.getByLabelText("The Matrix")).toBeDefined();
});

test("playing again after an Admin's stop starts a new plan", async () => {
  const server = matrixServer();
  openApp(PLAYER, server.transport);
  await screen.findByLabelText("The Matrix");
  ended(server, 1n, MESSAGE);
  await screen.findByRole("heading", { name: "An Admin stopped playback" });

  fireEvent.click(screen.getByRole("link", { name: "Back" }));
  const play = await screen.findByRole("link", { name: /^(?:Play|Resume)/u });
  // React Query keeps even a `gcTime: 0` suspense query a second after its player goes (#1636).
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 1000);
    });
  });
  fireEvent.click(play);

  await expect(screen.findByLabelText("The Matrix")).resolves.toBeDefined();
  expect(server.plans()).toBe(2);
});

test("a stop kept from before the Server restarted doesn't close a new plan given its id", async () => {
  const server = matrixServer();
  openApp("/libraries/1/movies/tmdb-603", server.transport);
  const play = await screen.findByRole("link", { name: /^(?:Play|Resume)/u });
  // Plan ids start again at 1 when the Server restarts.
  ended(server, 1n, MESSAGE);
  await eventsApplied(server);

  fireEvent.click(play);

  await expect(screen.findByLabelText("The Matrix")).resolves.toBeDefined();
  expect(screen.queryByRole("heading", { name: "An Admin stopped playback" })).toBeNull();
});
