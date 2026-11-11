import { Code, type ServiceImpl } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  HardwareAcceleration,
  ServerService,
  type UpdatePlaybackSettingsRequest,
} from "@/gen/nama/v1/server_pb";

import { admin, fakeServer, fill, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

/**
 * Opens the Playback page on a Server answering as `serverService` says, with no hardware and
 * saving whatever it's sent, and returns the saves it's sent.
 */
function openPlayback(serverService: Partial<ServiceImpl<typeof ServerService>>) {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openApp(
    "/admin/playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        updatePlaybackSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
        ...serverService,
      },
    }),
  );
  return updates;
}

test("an Admin sets the segment folder and the Transcodes at once on the Playback page", async () => {
  const updates = openPlayback({
    getPlaybackSettings: () => ({
      settings: { segmentFolder: "/srv/segments", maxTranscodes: 2 },
    }),
  });

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
  const updates = openPlayback({
    getPlaybackSettings: () => ({ settings: { maxTranscodes: 2 } }),
  });

  await screen.findByLabelText("Transcodes at once");
  fill("Transcodes at once", "");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates[0]?.settings?.maxTranscodes).toBeUndefined();
});

test("an Admin chooses hardware acceleration that passed its test", async () => {
  const updates = openPlayback({
    getPlaybackStatus: () => ({
      hardware: [{ api: HardwareAcceleration.QSV, toneMaps: true }],
    }),
    getPlaybackSettings: () => ({ settings: {} }),
  });

  const choice = await screen.findByRole("combobox", { name: "Hardware acceleration" });
  expect(choice).toHaveProperty("value", String(HardwareAcceleration.UNSPECIFIED));
  fireEvent.change(choice, { target: { value: String(HardwareAcceleration.QSV) } });
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([{ settings: { hardwareAcceleration: HardwareAcceleration.QSV } }]);
});

const twoGpus = {
  hardware: [
    {
      api: HardwareAcceleration.VAAPI,
      gpu: { id: "0000:03:00.0", name: "AMD Radeon RX 9070 XT" },
    },
    {
      api: HardwareAcceleration.VAAPI,
      gpu: { id: "0000:18:00.0", name: "integrated GPU (AMD Ryzen 9 7950X3D)" },
    },
    { api: HardwareAcceleration.NVENC },
  ],
};

/** The labels of the options the select labelled "Hardware acceleration" offers. */
async function hardwareOffered() {
  const select = await screen.findByRole("combobox", { name: "Hardware acceleration" });
  return within(select)
    .getAllByRole("option")
    .map((option) => option.textContent);
}

test("an Admin picks an API on one GPU from one option per pair", async () => {
  const updates = openPlayback({
    getPlaybackStatus: () => twoGpus,
    getPlaybackSettings: () => ({ settings: {} }),
  });

  expect(await hardwareOffered()).toEqual([
    "Automatic",
    "Off",
    "AMD Radeon RX 9070 XT · VAAPI",
    "Integrated GPU (AMD Ryzen 9 7950X3D) · VAAPI",
    "NVIDIA NVENC",
  ]);
  fireEvent.change(screen.getByRole("combobox", { name: "Hardware acceleration" }), {
    target: { value: `${HardwareAcceleration.VAAPI}@0000:18:00.0` },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([
    { settings: { hardwareAcceleration: HardwareAcceleration.VAAPI, gpu: { id: "0000:18:00.0" } } },
  ]);
});

test("a saved API with no GPU shows the first GPU it passed on chosen, which saving sends", async () => {
  const updates = openPlayback({
    getPlaybackStatus: () => twoGpus,
    getPlaybackSettings: () => ({
      settings: { hardwareAcceleration: HardwareAcceleration.VAAPI },
    }),
  });

  expect(
    await screen.findByRole("option", { name: "AMD Radeon RX 9070 XT · VAAPI", selected: true }),
  ).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([
    { settings: { hardwareAcceleration: HardwareAcceleration.VAAPI, gpu: { id: "0000:03:00.0" } } },
  ]);
});

test("a saved GPU that didn't pass is offered as not found and stays chosen", async () => {
  const updates = openPlayback({
    getPlaybackStatus: () => ({ hardware: twoGpus.hardware.slice(1) }),
    getPlaybackSettings: () => ({
      settings: {
        hardwareAcceleration: HardwareAcceleration.VAAPI,
        gpu: { id: "0000:03:00.0", name: "AMD Radeon RX 9070 XT" },
      },
    }),
  });

  expect(await hardwareOffered()).toEqual([
    "Automatic",
    "Off",
    "Integrated GPU (AMD Ryzen 9 7950X3D) · VAAPI",
    "NVIDIA NVENC",
    "AMD Radeon RX 9070 XT · VAAPI (not found)",
  ]);
  expect(
    screen.getByRole("option", {
      name: "AMD Radeon RX 9070 XT · VAAPI (not found)",
      selected: true,
    }),
  ).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([
    { settings: { hardwareAcceleration: HardwareAcceleration.VAAPI, gpu: { id: "0000:03:00.0" } } },
  ]);
});

test("a GPU the Server refuses is refused under the hardware acceleration", async () => {
  openPlayback({
    getPlaybackStatus: () => twoGpus,
    getPlaybackSettings: () => ({ settings: {} }),
    updatePlaybackSettings: () => {
      throw refused(
        Code.InvalidArgument,
        "the GPU must be one the hardware acceleration passed its test on",
        {
          reason: RefusalReason.GPU_NOT_OFFERED,
          field: "settings.gpu",
        },
      );
    },
  });

  await screen.findByRole("combobox", { name: "Hardware acceleration" });
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  await expectUnderField(
    "Hardware acceleration",
    "The GPU must be one the hardware acceleration passed its test on.",
  );
});

test("a saved hardware acceleration that didn't pass its test stays chosen", async () => {
  const updates = openPlayback({
    getPlaybackSettings: () => ({
      settings: { hardwareAcceleration: HardwareAcceleration.NVENC },
    }),
  });

  expect(await screen.findByRole("option", { name: "NVIDIA NVENC", selected: true })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([
    { settings: { hardwareAcceleration: HardwareAcceleration.NVENC } },
  ]);
});

test("a segment folder that isn't an absolute path is refused under its field", async () => {
  openPlayback({
    getPlaybackSettings: () => ({ settings: { maxTranscodes: 2 } }),
    updatePlaybackSettings: () => {
      throw refused(Code.InvalidArgument, "the segment folder must be an absolute path", {
        reason: RefusalReason.FOLDER_NOT_ABSOLUTE,
        field: "settings.segment_folder",
      });
    },
  });

  await screen.findByLabelText("Segment folder");
  fill("Segment folder", "shm/nama");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  await expectUnderField("Segment folder", "The segment folder must be an absolute path.");
});

test("a Transcodes at once the Server refuses as missing is refused under its field", async () => {
  openPlayback({
    getPlaybackSettings: () => ({ settings: { maxTranscodes: 2 } }),
    updatePlaybackSettings: () => {
      throw refused(Code.InvalidArgument, "the most Transcodes at once must be at least 1", {
        reason: RefusalReason.FIELD_REQUIRED,
        field: "settings.max_transcodes",
      });
    },
  });

  await screen.findByLabelText("Transcodes at once");
  fireEvent.click(screen.getByRole("button", { name: "Save playback settings" }));

  await expectUnderField("Transcodes at once", "The most Transcodes at once must be at least 1.");
});

test("an Admin sets the default Playback Limit for remote playback, which starts at Original", async () => {
  const updates = openPlayback({
    getPlaybackSettings: () => ({
      settings: {
        segmentFolder: "/srv/segments",
        maxTranscodes: 2,
        remotePlaybackLimit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL,
      },
    }),
  });
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
