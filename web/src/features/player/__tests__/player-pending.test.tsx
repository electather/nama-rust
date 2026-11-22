import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type PlanPlaybackRequest,
  PlaybackMethod,
  type PlaybackPlanSchema,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openTracks, pickIn, rowsOf } from "@/test-player";

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";
// Shown only after `getting-ready.tsx` waits 1 s, so finding it needs `test-app.tsx`'s raised wait.
const READY = "Getting this ready…";

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
        // oxlint-disable-next-line require-await -- it answers when the test says so
        planPlayback: async (request) =>
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
  // oxlint-disable-next-line require-await -- it hands over waitFor's promise
  const asked = async (n: number) =>
    waitFor(() => {
      const ask = held[n];
      if (!ask) {
        throw new Error(`plan request ${n} not sent yet`);
      }
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

  const getting = await screen.findByRole("status");
  expect(getting.textContent).toBe(READY);
  expect(screen.getByRole("heading", { name: "Alien" })).toBeDefined();
  expect(screen.getByRole("link", { name: "Back" }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-348",
  );

  const first = await server.asked(0);
  first.answer(directPlay("/media/versions/7", 1));

  const video = await screen.findByLabelText("Alien");
  expect(video.getAttribute("src")).toBe("/media/versions/7");
  expect(screen.queryByText(READY)).toBeNull();
});

test("choosing another audio Track shows it's getting ready until that plan comes, then plays it with the choice kept", async () => {
  const server = slowServer();
  openApp(PLAYER, server.transport);
  const first = await server.asked(0);
  first.answer(directPlay("/media/versions/7", 1));
  await screen.findByLabelText("Alien");

  pickIn("Audio", /Commentary/u);

  const second = await server.asked(1);
  expect(second.request.audioTrack).toBe(2);
  await expect(screen.findByText(READY)).resolves.toBeDefined();

  second.answer(directPlay("/media/versions/7?audio=2", 2));

  await waitFor(() => {
    expect(screen.getByLabelText("Alien").getAttribute("src")).toBe("/media/versions/7?audio=2");
  });
  expect(screen.queryByText(READY)).toBeNull();
  openTracks();
  expect(rowsOf("Audio").map((item) => item.getAttribute("aria-checked"))).toStrictEqual([
    "false",
    "true",
  ]);
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

  await expect(screen.findByText("This Version can’t play here")).resolves.toBeDefined();
  expect(screen.getByText("The Server couldn’t get it ready to play.")).toBeDefined();
  expect(screen.getAllByRole("link", { name: "Back" })).not.toHaveLength(0);
  expect(screen.queryByText("Something went wrong")).toBeNull();
});
