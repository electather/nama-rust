import { Code } from "@connectrpc/connect";
import { fireEvent, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { UpdatePlaybackSettingsRequest } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, fill, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

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

test("an Admin sets the segment folder and the Transcodes at once on the Playback page", async () => {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({
          settings: { segmentFolder: "/srv/segments", maxTranscodes: 2 },
        }),
        updatePlaybackSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );

  expect(await screen.findByLabelText("Segment folder")).toHaveProperty("value", "/srv/segments");
  expect(screen.getByLabelText("Transcodes at once")).toHaveProperty("value", "2");
  fill("Segment folder", "/dev/shm/nama");
  fill("Transcodes at once", "1");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([
    { settings: { segmentFolder: "/dev/shm/nama", maxTranscodes: 1 } },
  ]);
});

test("a segment folder that isn't an absolute path is refused under its field", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        updatePlaybackSettings: () => {
          throw refused(Code.InvalidArgument, "the segment folder must be an absolute path", {
            reason: RefusalReason.FOLDER_NOT_ABSOLUTE,
            field: "settings.segment_folder",
          });
        },
      },
    }),
  );

  await screen.findByLabelText("Segment folder");
  fill("Segment folder", "shm/nama");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  await expectUnderField("Segment folder", "The segment folder must be an absolute path.");
});
