import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, type ServiceImpl } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  HardwareAcceleration,
  type PlaybackSettingsSchema,
  type ServerService,
  type UpdatePlaybackSettingsRequest,
} from "@/gen/nama/v1/server_pb";
import { admin, fakeServer, fill } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { openSettings } from "@/test-settings";

/**
 * Opens Server Playback on a Server answering as `serverService` says, with no hardware, which
 * keeps whatever it's sent over `settings`, and returns the saves it's sent.
 */
function openPlayback(
  settings: MessageInitShape<typeof PlaybackSettingsSchema>,
  serverService: Partial<ServiceImpl<typeof ServerService>> = {},
) {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openSettings(
    "server-playback",
    fakeServer({
      signedIn: admin,
      serverService: {
        getPlaybackStatus: () => ({}),
        getPlaybackSettings: () => ({ settings: updates.at(-1)?.settings ?? settings }),
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

/** Types `value` into the field labelled `label` and leaves it, which saves it. */
function change(label: string, value: string) {
  fill(label, value);
  fireEvent.blur(screen.getByLabelText(label));
}

test("an Admin's segment folder, Keep for skipping back and Transcodes at once are each saved once its field is left", async () => {
  const updates = openPlayback({
    segmentFolder: "/srv/segments",
    keepBehindMinutes: 12,
    maxTranscodes: 2,
  });

  const folder = await screen.findByLabelText("Segment folder");
  expect(folder).toHaveProperty("value", "/srv/segments");
  expect(screen.getByLabelText("Keep for skipping back")).toHaveProperty("value", "12");
  expect(screen.getByLabelText("Transcodes at once")).toHaveProperty("value", "2");
  fill("Segment folder", "/dev/shm/nama");
  // Nothing is saved while it's typed.
  expect(updates).toStrictEqual([]);
  fireEvent.blur(folder);
  await expect.poll(() => updates).toHaveLength(1);
  change("Keep for skipping back", "3");
  await expect.poll(() => updates).toHaveLength(2);
  change("Transcodes at once", "1");

  await expect
    .poll(() => updates)
    .toMatchObject([
      { settings: { segmentFolder: "/dev/shm/nama", keepBehindMinutes: 12, maxTranscodes: 2 } },
      { settings: { segmentFolder: "/dev/shm/nama", keepBehindMinutes: 3, maxTranscodes: 2 } },
      { settings: { segmentFolder: "/dev/shm/nama", keepBehindMinutes: 3, maxTranscodes: 1 } },
    ]);
  // Leaving a field again saves nothing more.
  fireEvent.blur(folder);
  expect(updates).toHaveLength(3);
});

test("an Admin who empties Keep for skipping back and Transcodes at once sends them unset", async () => {
  const updates = openPlayback({ keepBehindMinutes: 12, maxTranscodes: 2 });

  await screen.findByLabelText("Transcodes at once");
  change("Keep for skipping back", "");
  await expect.poll(() => updates).toHaveLength(1);
  change("Transcodes at once", "");

  await expect.poll(() => updates).toHaveLength(2);
  expect(updates[1]?.settings?.keepBehindMinutes).toBeUndefined();
  expect(updates[1]?.settings?.maxTranscodes).toBeUndefined();
});

test("an Admin chooses hardware acceleration that passed its test, saved as it's chosen", async () => {
  const updates = openPlayback(
    {},
    {
      getPlaybackStatus: () => ({ hardware: [{ api: HardwareAcceleration.QSV, toneMaps: true }] }),
    },
  );

  const choice = await screen.findByRole("combobox", { name: "Hardware acceleration" });
  expect(choice).toHaveProperty("value", String(HardwareAcceleration.UNSPECIFIED));
  fireEvent.change(choice, { target: { value: String(HardwareAcceleration.QSV) } });

  await expect
    .poll(() => updates)
    .toMatchObject([{ settings: { hardwareAcceleration: HardwareAcceleration.QSV } }]);
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
  const updates = openPlayback({}, { getPlaybackStatus: () => twoGpus });

  await expect(hardwareOffered()).resolves.toStrictEqual([
    "Automatic",
    "Off",
    "AMD Radeon RX 9070 XT · VAAPI",
    "Integrated GPU (AMD Ryzen 9 7950X3D) · VAAPI",
    "NVIDIA NVENC",
  ]);
  fireEvent.change(screen.getByRole("combobox", { name: "Hardware acceleration" }), {
    target: { value: `${HardwareAcceleration.VAAPI}@0000:18:00.0` },
  });

  await expect
    .poll(() => updates)
    .toMatchObject([
      {
        settings: { hardwareAcceleration: HardwareAcceleration.VAAPI, gpu: { id: "0000:18:00.0" } },
      },
    ]);
});

test("a saved API with no GPU shows the first GPU it passed on chosen, which the next save sends", async () => {
  const updates = openPlayback(
    { hardwareAcceleration: HardwareAcceleration.VAAPI },
    { getPlaybackStatus: () => twoGpus },
  );

  await expect(
    screen.findByRole("option", { name: "AMD Radeon RX 9070 XT · VAAPI", selected: true }),
  ).resolves.toBeDefined();
  change("Transcodes at once", "3");

  await expect
    .poll(() => updates)
    .toMatchObject([
      {
        settings: { hardwareAcceleration: HardwareAcceleration.VAAPI, gpu: { id: "0000:03:00.0" } },
      },
    ]);
});

test("a saved GPU that didn't pass is offered as not found and stays chosen", async () => {
  const updates = openPlayback(
    {
      hardwareAcceleration: HardwareAcceleration.VAAPI,
      gpu: { id: "0000:03:00.0", name: "AMD Radeon RX 9070 XT" },
    },
    { getPlaybackStatus: () => ({ hardware: twoGpus.hardware.slice(1) }) },
  );

  await expect(hardwareOffered()).resolves.toStrictEqual([
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
  change("Transcodes at once", "3");

  await expect
    .poll(() => updates)
    .toMatchObject([
      {
        settings: { hardwareAcceleration: HardwareAcceleration.VAAPI, gpu: { id: "0000:03:00.0" } },
      },
    ]);
});

test("a GPU the Server refuses is refused under the hardware acceleration", async () => {
  openPlayback(
    {},
    {
      getPlaybackStatus: () => twoGpus,
      updatePlaybackSettings: () => {
        throw refused(
          Code.InvalidArgument,
          "the GPU must be one the hardware acceleration passed its test on",
          { reason: RefusalReason.GPU_NOT_OFFERED, field: "settings.gpu" },
        );
      },
    },
  );

  fireEvent.change(await screen.findByRole("combobox", { name: "Hardware acceleration" }), {
    target: { value: String(HardwareAcceleration.NVENC) },
  });

  await expectUnderField(
    "Hardware acceleration",
    "The GPU must be one the hardware acceleration passed its test on.",
  );
});

test("a saved hardware acceleration that didn't pass its test stays chosen", async () => {
  const updates = openPlayback({ hardwareAcceleration: HardwareAcceleration.NVENC });

  await expect(
    screen.findByRole("option", { name: "NVIDIA NVENC", selected: true }),
  ).resolves.toBeDefined();
  change("Transcodes at once", "3");

  await expect
    .poll(() => updates)
    .toMatchObject([{ settings: { hardwareAcceleration: HardwareAcceleration.NVENC } }]);
});

test.each([
  ["isn't an absolute path", RefusalReason.FOLDER_NOT_ABSOLUTE, "shm/nama"],
  ["the Server can't write", RefusalReason.FOLDER_NOT_WRITABLE, "/proc/nama"],
])("a segment folder that %s is refused under its field", async (_case, reason, folder) => {
  openPlayback(
    { maxTranscodes: 2 },
    {
      updatePlaybackSettings: () => {
        throw refused(Code.InvalidArgument, "the segment folder is no good", {
          reason,
          field: "settings.segment_folder",
        });
      },
    },
  );

  await screen.findByLabelText("Segment folder");
  change("Segment folder", folder);

  await expectUnderField("Segment folder", "The segment folder is no good.");
});

test.each([
  ["Transcodes at once", "max_transcodes"],
  ["Keep for skipping back", "keep_behind_minutes"],
])("a %s the Server refuses as missing is refused under its field", async (label, field) => {
  openPlayback(
    {},
    {
      updatePlaybackSettings: () => {
        throw refused(Code.InvalidArgument, "it must be at least 1", {
          reason: RefusalReason.FIELD_REQUIRED,
          field: `settings.${field}`,
        });
      },
    },
  );

  await screen.findByLabelText(label);
  change(label, "0");

  await expectUnderField(label, "It must be at least 1.");
});

test("an Admin sets the default Playback Limit for remote playback, which starts at Original", async () => {
  const updates = openPlayback({
    segmentFolder: "/srv/segments",
    maxTranscodes: 2,
    remotePlaybackLimit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL,
  });
  const limit = "Default Playback Limit for remote playback";

  const select = await screen.findByLabelText<HTMLSelectElement>(limit);
  const offered = [...select.options].map((option) => option.text);
  expect(select.selectedOptions[0]?.text).toBe("Original");
  fireEvent.change(select, { target: { value: String(PlaybackLimit.PLAYBACK_LIMIT_720P) } });

  await expect
    .poll(() => updates)
    .toMatchObject([
      {
        settings: {
          segmentFolder: "/srv/segments",
          maxTranscodes: 2,
          remotePlaybackLimit: PlaybackLimit.PLAYBACK_LIMIT_720P,
        },
      },
    ]);
  expect(offered).toStrictEqual(["Original", "1080p", "720p", "480p"]);
});
