import { screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, openApp } from "./test-app";

test("an Admin sees the path and version of the ffmpeg and ffprobe the Server found", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverSettings: {
        getPlaybackStatus: () => ({
          ffmpeg: { path: "/usr/bin/ffmpeg", version: "7.1.1" },
          ffprobe: { path: "/usr/bin/ffprobe", version: "7.1.1" },
        }),
      },
    }),
  );

  expect(await screen.findByText("/usr/bin/ffmpeg")).toBeDefined();
  expect(screen.getByText("/usr/bin/ffprobe")).toBeDefined();
  expect(screen.getAllByText("Version 7.1.1")).toHaveLength(2);
  expect(screen.queryByText(/can only Direct Play/u)).toBeNull();
});

test("without ffmpeg the Playback page says it wasn’t found and that the Server can only Direct Play", async () => {
  openApp(
    "/admin/playback",
    fakeServer({ signedIn: admin, serverSettings: { getPlaybackStatus: () => ({}) } }),
  );

  expect(await screen.findByText("ffmpeg wasn’t found on the PATH.")).toBeDefined();
  expect(screen.getByText("ffprobe wasn’t found on the PATH.")).toBeDefined();
  expect(screen.getByText(/can only Direct Play/u)).toBeDefined();
});

test("a User who manages settings but isn’t an Admin can’t open the Playback page", async () => {
  openApp("/admin/playback", fakeServer({ signedIn: [Permission.MANAGE_SETTINGS] }));

  expect(await screen.findByRole("heading", { name: "You can’t open this page" })).toBeDefined();
});
