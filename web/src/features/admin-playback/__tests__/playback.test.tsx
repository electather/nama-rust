import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { HardwareAcceleration, ToneMapping } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

test("an Admin sees the path and version of the ffmpeg and ffprobe the Server found", async () => {
  openSettings(
    "server-playback",
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

  await expect(screen.findByText("/usr/bin/ffmpeg")).resolves.toBeDefined();
  expect(screen.getByText("/usr/bin/ffprobe")).toBeDefined();
  expect(screen.getAllByText("version 7.1.1")).toHaveLength(2);
  expect(screen.queryByText(/can only Direct Play/u)).toBeNull();
});

test("without a usable ffmpeg Server Playback says why and that the Server can only Direct Play", async () => {
  openSettings(
    "server-playback",
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

  await expect(
    screen.findByText(
      "ffmpeg 4.3.1 at /usr/bin/ffmpeg is older than 4.4, the oldest this Server supports",
    ),
  ).resolves.toBeDefined();
  expect(
    screen.getByText("ffprobe wasn't found at the configured path /opt/ffprobe"),
  ).toBeDefined();
  expect(screen.getByText(/can only Direct Play/u)).toBeDefined();
});

test("without ffmpeg or hardware Server Playback doesn't say hardware passed its test", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ffmpegProblem: "ffmpeg wasn't found on the PATH",
          ffprobeProblem: "ffprobe wasn't found on the PATH",
        }),
      },
    }),
  );

  await expect(
    screen.findByText(/What Transcodes can run on besides software/u),
  ).resolves.toBeDefined();
  expect(screen.queryByText(/hardware below passed its test/u)).toBeNull();
});

const ffmpegFound = {
  ffmpeg: { path: "/usr/bin/ffmpeg", version: "7.1.1" },
  ffprobe: { path: "/usr/bin/ffprobe", version: "7.1.1" },
};

test("an Admin sees the hardware acceleration that passed its test and whether it tone-maps", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hardware: [
            {
              api: HardwareAcceleration.NVENC,
              toneMapping: ToneMapping.GPU,
              automatic: true,
              hevc: true,
            },
            { api: HardwareAcceleration.VAAPI, toneMapping: ToneMapping.SOFTWARE },
          ],
          hasLibx264: true,
          hasZscale: true,
          canTranscode: true,
        }),
      },
    }),
  );

  await expect(screen.findByRole("heading", { name: "NVIDIA NVENC" })).resolves.toBeDefined();
  expect(screen.getByRole("heading", { name: "VAAPI" })).toBeDefined();
  expect(screen.getByText(/on the GPU too\. Automatic picks this one\./u)).toBeDefined();
  expect(screen.getByText(/in software, between decoding/u)).toBeDefined();
  expect(screen.queryByText(/Transcodes run in software/u)).toBeNull();
});

test("an Admin sees which hardware can’t make HEVC", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hardware: [
            { api: HardwareAcceleration.QSV, toneMapping: ToneMapping.GPU, hevc: true },
            { api: HardwareAcceleration.VAAPI, toneMapping: ToneMapping.GPU },
          ],
          hasLibx264: true,
          canTranscode: true,
        }),
      },
    }),
  );

  const vaapi = await screen.findByRole("heading", { name: "VAAPI" });
  expect(vaapi.nextElementSibling?.textContent).toMatch(/Can’t make HEVC/u);
  expect(screen.getAllByText(/make HEVC/u)).toHaveLength(1);
});

test("an Admin sees a row for each API on each GPU that passed its test", async () => {
  const dedicated = { id: "0000:03:00.0", name: "AMD Radeon RX 9070 XT" };
  const integrated = { id: "0000:18:00.0", name: "integrated GPU (AMD Ryzen 9 7950X3D)" };
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hardware: [
            { api: HardwareAcceleration.VAAPI, gpu: dedicated, automatic: true },
            { api: HardwareAcceleration.VAAPI, gpu: integrated },
          ],
          hasLibx264: true,
          canTranscode: true,
        }),
      },
    }),
  );

  const rows = await screen.findAllByRole("heading", { name: /· VAAPI$/u });
  expect(rows.map((row) => row.textContent)).toStrictEqual([
    "AMD Radeon RX 9070 XT · VAAPI",
    "Integrated GPU (AMD Ryzen 9 7950X3D) · VAAPI",
  ]);
});

test("without hardware acceleration Server Playback says Transcodes run in software", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hasLibx264: true,
          hasZscale: true,
          canTranscode: true,
        }),
      },
    }),
  );

  await expect(screen.findByText(/Transcodes run in software/u)).resolves.toBeDefined();
});

test("an ffmpeg without libx264 makes Server Playback say the Server can’t Transcode", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: { getPlaybackStatus: () => ({ ...ffmpegFound, hasZscale: true }) },
    }),
  );

  await expect(
    screen.findByText(/ffmpeg lacks libx264, so it can’t Transcode, only Direct Play/u),
  ).resolves.toBeDefined();
  expect(screen.queryByText(/Transcodes run in software/u)).toBeNull();
});

test("an ffmpeg without libx264 lists hardware that passed without saying Transcodes run on it", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hardware: [
            { api: HardwareAcceleration.NVENC, toneMapping: ToneMapping.GPU, automatic: true },
          ],
          hasZscale: true,
        }),
      },
    }),
  );

  await expect(screen.findByRole("heading", { name: "NVIDIA NVENC" })).resolves.toBeDefined();
  expect(
    screen.getByText(/no Transcode runs on it, .* until the Server’s ffmpeg has libx264/u),
  ).toBeDefined();
  expect(screen.queryByText(/What Transcodes can run on besides software/u)).toBeNull();
  expect(screen.queryByText(/Tone-maps HDR to SDR/u)).toBeNull();
  expect(screen.queryByText(/Automatic picks this one/u)).toBeNull();
});

test("without ffprobe Server Playback lists hardware that passed without saying Transcodes run on it", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ffmpeg: ffmpegFound.ffmpeg,
          ffprobeProblem: "ffprobe wasn't found on the PATH",
          hardware: [
            { api: HardwareAcceleration.NVENC, toneMapping: ToneMapping.GPU, automatic: true },
          ],
          hasLibx264: true,
          hasZscale: true,
        }),
      },
    }),
  );

  await expect(screen.findByRole("heading", { name: "NVIDIA NVENC" })).resolves.toBeDefined();
  expect(screen.getByText("Passed")).toBeDefined();
  expect(screen.getByText(/no Transcode runs on it until the Server finds ffprobe/u)).toBeDefined();
  expect(screen.queryByText(/What Transcodes can run on besides software/u)).toBeNull();
  expect(screen.queryByText(/Tone-maps HDR to SDR/u)).toBeNull();
  expect(screen.queryByText(/Automatic picks this one/u)).toBeNull();
});

test("without ffprobe or libx264 Server Playback says hardware waits for both", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ffmpeg: ffmpegFound.ffmpeg,
          ffprobeProblem: "ffprobe wasn't found on the PATH",
          hardware: [
            { api: HardwareAcceleration.NVENC, toneMapping: ToneMapping.GPU, automatic: true },
          ],
          hasZscale: true,
        }),
      },
    }),
  );

  await expect(
    screen.findByText(
      /no Transcode runs on it until the Server finds ffprobe and its ffmpeg has libx264/u,
    ),
  ).resolves.toBeDefined();
  expect(screen.queryByText(/Automatic picks this one/u)).toBeNull();
});

test("an ffmpeg without zscale makes Server Playback say it can’t tone-map HDR in software", async () => {
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          ...ffmpegFound,
          hardware: [{ api: HardwareAcceleration.VAAPI, toneMapping: ToneMapping.NONE }],
          hasLibx264: true,
          canTranscode: true,
        }),
      },
    }),
  );

  await expect(
    screen.findByText(/ffmpeg lacks zscale, so a Transcode can’t tone-map HDR to SDR in software/u),
  ).resolves.toBeDefined();
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
  "without %s Server Playback says it can only Direct Play, not what ffmpeg lacks",
  async (_missing, status) => {
    openSettings(
      "server-playback",
      fakeServer({ signedIn: admin, serverService: { getPlaybackStatus: () => status } }),
    );

    await expect(screen.findByText(/can only Direct Play/u)).resolves.toBeDefined();
    expect(screen.queryByText(/libx264/u)).toBeNull();
    expect(screen.queryByText(/zscale/u)).toBeNull();
    expect(screen.queryByText(/Transcodes run in software/u)).toBeNull();
  },
);

test("a User who manages settings but isn’t an Admin gets their own Playback instead", async () => {
  openSettings(
    "server-playback",
    fakeServer({ signedIn: [...newUser, Permission.MANAGE_SETTINGS] }),
  );

  await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
  expect(screen.queryByRole("link", { name: "Server Playback" })).toBeNull();
});

test("searching Settings for ffmpeg finds it in Server Playback", async () => {
  openSettings("playback", fakeServer({ signedIn: admin }));

  fireEvent.change(await screen.findByRole("combobox", { name: "Search settings" }), {
    target: { value: "ffmpeg" },
  });

  expect(screen.getAllByRole("option").map((option) => option.textContent)).toStrictEqual([
    "Server Playbackffmpeg and ffprobe",
  ]);
});
