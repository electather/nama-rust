import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackMethod,
  type PlaybackPlanSchema,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const READY = "Getting this ready…";
/** Longer than the player waits before saying it's getting ready. */
const NOTICED = { timeout: 3000 };

type Plan = MessageInitShape<typeof PlaybackPlanSchema>;

const audio = (channels: number) => ({ case: "audio", value: { channels } }) as const;

/**
 * A fake Server with Alien in Library 1, offering its English audio and a commentary, whose
 * `planPlayback` answers only when the test calls `answer` on the request it holds.
 */
function slowServer() {
  const held: { request: PlanPlaybackRequest; answer: (plan: Plan) => void }[] = [];
  const tracks = [
    { codec: "h264", kind: { case: "video", value: {} } },
    { codec: "aac", language: "eng", default: true, kind: audio(6) },
    { codec: "aac", language: "eng", title: "Commentary", kind: audio(2) },
  ] as const;
  const transport = fakeServer(
    { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        getMovie: () => ({
          movie: {
            id: { namespace: "tmdb", value: "348" },
            title: "Alien",
            versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n, tracks: [...tracks] }],
          },
        }),
      });
      router.service(PlaybackService, {
        planPlayback: (request) =>
          new Promise((resolve) => {
            held.push({
              request,
              answer: (plan) => {
                resolve({ plan });
              },
            });
          }),
      });
    },
  );
  /** The `n`th plan request, once the player has sent it. */
  const asked = (n: number) =>
    waitFor(() => {
      const ask = held[n];
      if (!ask) throw new Error(`plan request ${n} not sent yet`);
      return ask;
    });
  return { transport, asked };
}

const directPlay = (url: string, audioTrack: number): Plan => ({
  method: PlaybackMethod.DIRECT_PLAY,
  url,
  audioTrack,
  audioTracks: [1, 2],
});

test("the player shows the title, Back and that it's getting ready until the first plan comes, then plays it", async () => {
  const server = slowServer();
  openApp(PLAYER, server.transport);

  expect((await screen.findByRole("status", {}, NOTICED)).textContent).toBe(READY);
  expect(screen.getByRole("heading", { name: "Alien" })).toBeDefined();
  expect(screen.getByRole("link", { name: "Back" }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-348",
  );

  (await server.asked(0)).answer(directPlay("/media/versions/7", 1));

  const video = await screen.findByLabelText("Alien");
  expect(video.getAttribute("src")).toBe("/media/versions/7");
  expect(screen.queryByText(READY)).toBeNull();
});

test("choosing another audio Track shows it's getting ready until that plan comes, then plays it with the choice kept", async () => {
  const server = slowServer();
  openApp(PLAYER, server.transport);
  (await server.asked(0)).answer(directPlay("/media/versions/7", 1));
  await screen.findByLabelText("Alien");

  fireEvent.click(screen.getByRole("button", { name: "Audio" }));
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Commentary/u }));

  const second = await server.asked(1);
  expect(second.request.audioTrack).toBe(2);
  expect(await screen.findByText(READY, {}, NOTICED)).toBeDefined();

  second.answer(directPlay("/media/versions/7?audio=2", 2));

  await waitFor(() => {
    expect(screen.getByLabelText("Alien").getAttribute("src")).toBe("/media/versions/7?audio=2");
  });
  expect(screen.queryByText(READY)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Audio" }));
  expect(
    screen.getAllByRole("menuitemradio").map((item) => item.getAttribute("aria-checked")),
  ).toEqual(["false", "true"]);
});

test("a Server that fails to plan the Version says it couldn't get it ready, not that something went wrong", async () => {
  const failing = fakeServer(
    { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        getMovie: () => ({
          movie: { title: "Alien", versions: [{ id: 7n, fileName: "Alien.mkv", size: 1n }] },
        }),
      });
      router.service(PlaybackService, {
        planPlayback: () => {
          throw new ConnectError("the Server failed", Code.Internal);
        },
      });
    },
  );
  openApp(PLAYER, failing);

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.getByText("The Server couldn’t get it ready to play.")).toBeDefined();
  expect(screen.getByRole("link", { name: "Back" })).toBeDefined();
  expect(screen.queryByText("Something went wrong")).toBeNull();
});
