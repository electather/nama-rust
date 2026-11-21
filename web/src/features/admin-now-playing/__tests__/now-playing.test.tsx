import type { MessageInitShape } from "@bufbuild/protobuf";
import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vite-plus/test";

import { PlaybackMethod, PlaybackReasonKind } from "@/gen/nama/v1/playback_pb";
import {
  HardwareAcceleration,
  type PlaySchema,
  type StopPlayRequest,
} from "@/gen/nama/v1/server_pb";
import { admin, eventFeed, fakeServer, fill, openApp } from "@/test-app";

type Play = MessageInitShape<typeof PlaySchema>;

const darkOnTv: Play = {
  playback: {
    planId: 7n,
    userId: 2n,
    deviceId: 5n,
    method: PlaybackMethod.TRANSCODE,
    reasons: [
      {
        kind: PlaybackReasonKind.DEVICE_CANT_DECODE_VIDEO,
        params: { codec: "hevc" },
        decided: true,
      },
    ],
  },
  title: "Dark S01E02",
  positionMs: 754_000n,
  durationMs: 3_125_000n,
  transcodesOn: {
    api: HardwareAcceleration.VAAPI,
    gpu: { id: "0000:03:00.0", name: "AMD Radeon RX 9070 XT" },
  },
};

/**
 * Opens the page on a fake Server playing `plays` as they are at each call. A stop ends its play
 * and is kept in `stopped`.
 */
function openNowPlaying(plays: Play[], events = eventFeed(), stopped: StopPlayRequest[] = []) {
  const stopPlay = (request: StopPlayRequest) => {
    stopped.push(request);
    plays.splice(
      plays.findIndex((play) => play.playback?.planId === request.planId),
      1,
    );
    return {};
  };
  openApp(
    "/admin/now-playing",
    fakeServer(
      {
        signedIn: admin,
        events: events.stream,
        serverService: { listPlays: () => ({ plays }), stopPlay },
      },
      {},
      {
        listUsers: () => ({
          users: [
            { id: 1n, name: "Sam" },
            { id: 2n, name: "Ada" },
          ],
        }),
        listDevices: ({ userId }) => ({
          devices: userId === 2n ? [{ id: 5n, name: "Living room TV" }] : [],
        }),
      },
    ),
  );
}

describe("the Now Playing page", () => {
  test("shows a play going on as it opens: who, where, what, how, why, how far and on what", async () => {
    openNowPlaying([darkOnTv]);

    await expect(screen.findByText("Ada on Living room TV")).resolves.toBeDefined();
    expect(screen.getByText("Dark S01E02")).toBeDefined();
    expect(screen.getByText("Transcode on AMD Radeon RX 9070 XT · VAAPI")).toBeDefined();
    expect(screen.getByText("12:34 of 52:05")).toBeDefined();
    expect(
      screen.getByText(
        "This Device can’t decode HEVC video, so it can’t be Direct Played or Remuxed.",
      ),
    ).toBeDefined();
  });

  test("follows a play that starts, falls back to the CPU and ends, without a reload", async () => {
    const plays: Play[] = [];
    const events = eventFeed();
    openNowPlaying(plays, events);
    await expect(screen.findByText("Nothing is playing.")).resolves.toBeDefined();

    plays.push(darkOnTv);
    events.send({ event: { case: "playbackStarted", value: { playback: darkOnTv.playback } } });
    await expect(screen.findByText("Dark S01E02")).resolves.toBeDefined();

    plays[0] = { ...darkOnTv, transcodesOn: { api: HardwareAcceleration.OFF } };
    events.send({ event: { case: "playbackChanged", value: { playback: darkOnTv.playback } } });
    await expect(screen.findByText("Transcode on the CPU")).resolves.toBeDefined();

    plays.pop();
    events.send({ event: { case: "playbackEnded", value: { playback: darkOnTv.playback } } });
    await expect(screen.findByText("Nothing is playing.")).resolves.toBeDefined();
    expect(screen.queryByText("Dark S01E02")).toBeNull();
  });

  test("stop with a message ends the play, and it leaves the page", async () => {
    const stopped: StopPlayRequest[] = [];
    openNowPlaying([darkOnTv], eventFeed(), stopped);

    fireEvent.click(
      await screen.findByRole("button", { name: "Stop Dark S01E02 for Ada on Living room TV" }),
    );
    const dialog = screen.getByRole("alertdialog", { name: "Stop Dark S01E02?" });
    fill("Message (optional)", "Bedtime!");
    fireEvent.click(within(dialog).getByRole("button", { name: "Stop" }));

    await expect(screen.findByText("Nothing is playing.")).resolves.toBeDefined();
    expect(stopped.map(({ planId, message }) => ({ planId, message }))).toStrictEqual([
      { planId: 7n, message: "Bedtime!" },
    ]);
  });
});
