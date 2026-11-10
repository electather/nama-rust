import { screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, openApp } from "./test-app";

test("an Admin sees the path and version of the ffmpeg and ffprobe the Server found", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ffmpeg: { path: "/usr/bin/ffmpeg", version: "7.1.1" },
          ffprobe: { path: "/usr/bin/ffprobe", version: "7.1.1" },
        }),
      },
    }),
  );

  expect(await screen.findByText("/usr/bin/ffmpeg")).toBeDefined();
  expect(screen.getByText("/usr/bin/ffprobe")).toBeDefined();
  expect(screen.getAllByText("version 7.1.1")).toHaveLength(2);
  expect(screen.queryByText(/can only Direct Play/u)).toBeNull();
});

test("without a usable ffmpeg the Playback page says why and that the Server can only Direct Play", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ffmpegProblem:
            "ffmpeg 4.3.1 at /usr/bin/ffmpeg is older than 4.4, the oldest this Server supports",
          ffprobeProblem: "ffprobe wasn't found at the configured path /opt/ffprobe",
        }),
      },
    }),
  );

  expect(
    await screen.findByText(
      "ffmpeg 4.3.1 at /usr/bin/ffmpeg is older than 4.4, the oldest this Server supports",
    ),
  ).toBeDefined();
  expect(
    screen.getByText("ffprobe wasn't found at the configured path /opt/ffprobe"),
  ).toBeDefined();
  expect(screen.getByText(/can only Direct Play/u)).toBeDefined();
});

test("a User who manages settings but isn’t an Admin can’t open the Playback page", async () => {
  openApp("/admin/playback", fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }));

  expect(await screen.findByRole("heading", { name: "You can’t open this page" })).toBeDefined();
});
