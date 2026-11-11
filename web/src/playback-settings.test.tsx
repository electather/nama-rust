import { Code } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { HardwareAcceleration, type UpdatePlaybackSettingsRequest } from "@/gen/nama/v1/server_pb";

import { admin, fakeServer, fill, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

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

test("an Admin who leaves Transcodes at once empty sends it unset", async () => {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({ settings: { maxTranscodes: 2 } }),
        updatePlaybackSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );

  await screen.findByLabelText("Transcodes at once");
  fill("Transcodes at once", "");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates[0]?.settings?.maxTranscodes).toBeUndefined();
});

test("an Admin chooses hardware acceleration that passed its test", async () => {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          hardware: [{ api: HardwareAcceleration.QSV, toneMaps: true }],
        }),
        getPlaybackSettings: () => ({ settings: {} }),
        updatePlaybackSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );

  const choice = await screen.findByRole("combobox", { name: "Hardware acceleration" });
  expect(choice).toHaveProperty("value", String(HardwareAcceleration.UNSPECIFIED));
  fireEvent.change(choice, { target: { value: String(HardwareAcceleration.QSV) } });
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([{ settings: { hardwareAcceleration: HardwareAcceleration.QSV } }]);
});

test("an API that passed on two GPUs is offered once", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({
          hardware: [
            { api: HardwareAcceleration.VAAPI, gpu: { id: "0000:03:00.0", name: "AMD GPU" } },
            { api: HardwareAcceleration.VAAPI, gpu: { id: "0000:18:00.0", name: "AMD iGPU" } },
          ],
        }),
        getPlaybackSettings: () => ({ settings: {} }),
      },
    }),
  );

  const choice = await screen.findByRole("combobox", { name: "Hardware acceleration" });

  expect(
    within(choice)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["Automatic", "Off", "VAAPI"]);
});

test("a saved hardware acceleration that didn't pass its test stays chosen", async () => {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({
          settings: { hardwareAcceleration: HardwareAcceleration.NVENC },
        }),
        updatePlaybackSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );

  expect(await screen.findByRole("option", { name: "NVIDIA NVENC", selected: true })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([
    { settings: { hardwareAcceleration: HardwareAcceleration.NVENC } },
  ]);
});

test("a segment folder that isn't an absolute path is refused under its field", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({ settings: { maxTranscodes: 2 } }),
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

test("a Transcodes at once the Server refuses as missing is refused under its field", async () => {
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({ settings: { maxTranscodes: 2 } }),
        updatePlaybackSettings: () => {
          throw refused(Code.InvalidArgument, "the most Transcodes at once must be at least 1", {
            reason: RefusalReason.FIELD_REQUIRED,
            field: "settings.max_transcodes",
          });
        },
      },
    }),
  );

  await screen.findByLabelText("Transcodes at once");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  await expectUnderField("Transcodes at once", "The most Transcodes at once must be at least 1.");
});

test("an Admin sets the default Playback Limit for remote playback, which starts at Original", async () => {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({
          settings: {
            segmentFolder: "/srv/segments",
            maxTranscodes: 2,
            remotePlaybackLimit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL,
          },
        }),
        updatePlaybackSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );
  const limit = "Default Playback Limit for remote playback";

  const select = await screen.findByLabelText<HTMLSelectElement>(limit);
  const offered = [...select.options].map((option) => option.text);
  expect(select.selectedOptions[0]?.text).toBe("Original");
  fireEvent.change(select, { target: { value: String(PlaybackLimit.PLAYBACK_LIMIT_720P) } });
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(offered).toEqual(["Original", "1080p", "720p", "480p"]);
  expect(updates).toMatchObject([
    {
      settings: {
        segmentFolder: "/srv/segments",
        maxTranscodes: 2,
        remotePlaybackLimit: PlaybackLimit.PLAYBACK_LIMIT_720P,
      },
    },
  ]);
});
