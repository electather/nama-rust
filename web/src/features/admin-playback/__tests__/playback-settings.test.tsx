import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, type ServiceImpl } from "@connectrpc/connect";
import { fireEvent, screen, waitFor } from "@testing-library/react";
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
import { dropdown, offered, openSettings, pick } from "@/test-settings";

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

test("pressing Enter in a field saves it", async () => {
  const updates = openPlayback({});

  await screen.findByLabelText("Segment folder");
  fill("Segment folder", "/dev/shm/nama");
  fireEvent.keyDown(screen.getByLabelText("Segment folder"), { key: "Enter" });

  await expect
    .poll(() => updates)
    .toMatchObject([{ settings: { segmentFolder: "/dev/shm/nama" } }]);
});

test("a change made while one is saving is saved after it, so the newer one is kept", async () => {
  const sent: (number | undefined)[] = [];
  const finish: (() => void)[] = [];
  openPlayback(
    {},
    {
      updatePlaybackSettings: async (request) => {
        sent.push(request.settings?.maxTranscodes);
        await new Promise<void>((resolve) => {
          finish.push(resolve);
        });
        return { settings: request.settings };
      },
    },
  );

  await screen.findByLabelText("Transcodes at once");
  change("Transcodes at once", "3");
  await expect.poll(() => sent).toStrictEqual([3]);
  change("Transcodes at once", "4");
  // Not sent while the first is saving.
  await new Promise((resolve) => {
    setTimeout(resolve, 50);
  });
  expect(sent).toStrictEqual([3]);
  finish[0]?.();

  await expect.poll(() => sent).toStrictEqual([3, 4]);
  finish[1]?.();
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
  expect(choice.textContent).toBe("Automatic");
  pick("Hardware acceleration", "Intel QSV");

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

/** The labels of the options the dropdown labelled "Hardware acceleration" offers. */
async function hardwareOffered() {
  await screen.findByRole("combobox", { name: "Hardware acceleration" });
  return offered("Hardware acceleration");
}

/** Waits for the dropdown labelled "Hardware acceleration" to show `label` chosen. */
async function hardwareChosen(label: string) {
  await waitFor(() => {
    expect(dropdown("Hardware acceleration").textContent).toBe(label);
  });
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
  pick("Hardware acceleration", "Integrated GPU (AMD Ryzen 9 7950X3D) · VAAPI");

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

  await hardwareChosen("AMD Radeon RX 9070 XT · VAAPI");
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
  await hardwareChosen("AMD Radeon RX 9070 XT · VAAPI (not found)");
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

  await screen.findByRole("combobox", { name: "Hardware acceleration" });
  pick("Hardware acceleration", "NVIDIA NVENC");

  await expectUnderField(
    "Hardware acceleration",
    "The GPU must be one the hardware acceleration passed its test on.",
  );
});

test("a saved hardware acceleration that didn't pass its test stays chosen", async () => {
  const updates = openPlayback({ hardwareAcceleration: HardwareAcceleration.NVENC });

  await hardwareChosen("NVIDIA NVENC");
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

test("a refused segment folder stays typed under its refusal, and a later change saves what's saved with it", async () => {
  const sent: (string | undefined)[] = [];
  // Only the first save is refused.
  const firstAnswer = [
    () => {
      throw refused(Code.InvalidArgument, "the segment folder is no good", {
        reason: RefusalReason.FOLDER_NOT_WRITABLE,
        field: "settings.segment_folder",
      });
    },
  ];
  openPlayback(
    { segmentFolder: "/srv/segments" },
    {
      updatePlaybackSettings: (request) => {
        sent.push(request.settings?.segmentFolder);
        firstAnswer.shift()?.();
        return { settings: request.settings };
      },
    },
  );

  await screen.findByLabelText("Segment folder");
  change("Segment folder", "/proc/nama");
  await expectUnderField("Segment folder", "The segment folder is no good.");
  expect(screen.getByLabelText("Segment folder")).toHaveProperty("value", "/proc/nama");
  change("Transcodes at once", "3");

  await expect.poll(() => sent).toStrictEqual(["/proc/nama", "/srv/segments"]);
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

  const select = await screen.findByRole("combobox", { name: limit });
  const limits = await offered(limit);
  expect(select.textContent).toBe("Original");
  pick(limit, "720p");

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
  expect(limits).toStrictEqual(["Original", "1080p", "720p", "480p"]);
});
