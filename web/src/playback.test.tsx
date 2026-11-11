import { screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { HardwareAcceleration } from "@/gen/nama/v1/server_pb";
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

test("an Admin sees the hardware acceleration that passed its test and whether it tone-maps", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          hardware: [
            { api: HardwareAcceleration.NVENC, toneMaps: true },
            { api: HardwareAcceleration.VAAPI, toneMaps: false },
          ],
          hasLibx264: true,
          hasZscale: true,
        }),
      },
    }),
  );

  expect(await screen.findByRole("heading", { name: "NVIDIA NVENC" })).toBeDefined();
  expect(screen.getByRole("heading", { name: "VAAPI" })).toBeDefined();
  expect(screen.getByText(/on the GPU too\. Automatic picks this one\./u)).toBeDefined();
  expect(screen.getByText(/in software, between decoding/u)).toBeDefined();
  expect(screen.queryByText(/Transcodes run in software/u)).toBeNull();
});

const ffmpegFound = {
  ffmpeg: { path: "/usr/bin/ffmpeg", version: "7.1.1" },
  ffprobe: { path: "/usr/bin/ffprobe", version: "7.1.1" },
};

test("without hardware acceleration the Playback page says Transcodes run in software", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({ ...ffmpegFound, hasLibx264: true, hasZscale: true }),
      },
    }),
  );

  expect(await screen.findByText(/Transcodes run in software/u)).toBeDefined();
});

test("an ffmpeg without libx264 makes the Playback page say the Server can’t Transcode", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: { getPlaybackStatus: () => ({ ...ffmpegFound, hasZscale: true }) },
    }),
  );

  expect(
    await screen.findByText(/ffmpeg lacks libx264, so it can’t Transcode, only Direct Play/u),
  ).toBeDefined();
  expect(screen.queryByText(/Transcodes run in software/u)).toBeNull();
});

test("an ffmpeg without zscale makes the Playback page say it can’t tone-map HDR in software", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hardware: [{ api: HardwareAcceleration.VAAPI, toneMaps: false }],
          hasLibx264: true,
        }),
      },
    }),
  );

  expect(
    await screen.findByText(
      /ffmpeg lacks zscale, so a Transcode can’t tone-map HDR to SDR in software/u,
    ),
  ).toBeDefined();
  expect(screen.getByText(/Can’t tone-map HDR to SDR, on the GPU or in software/u)).toBeDefined();
  expect(screen.queryByText(/Tone-maps HDR to SDR in software/u)).toBeNull();
});

test.each([
  ["ffmpeg", { ffmpegProblem: "ffmpeg wasn't found on PATH" }],
  [
    "ffprobe",
    {
      ffmpeg: ffmpegFound.ffmpeg,
      ffprobeProblem: "ffprobe wasn't found on PATH",
      hasLibx264: true,
    },
  ],
])(
  "without %s the Playback page says it can only Direct Play, not what ffmpeg lacks",
  async (_missing, status) => {
    openApp(
      "/admin/playback",
      fakeServer({ signedIn: admin, serverService: { getPlaybackStatus: () => status } }),
    );

    expect(await screen.findByText(/can only Direct Play/u)).toBeDefined();
    expect(screen.queryByText(/libx264/u)).toBeNull();
    expect(screen.queryByText(/zscale/u)).toBeNull();
    expect(screen.queryByText(/Transcodes run in software/u)).toBeNull();
  },
);

test("a User who manages settings but isn’t an Admin can’t open the Playback page", async () => {
  openApp("/admin/playback", fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }));

  expect(await screen.findByRole("heading", { name: "You can’t open this page" })).toBeDefined();
});
