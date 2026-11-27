import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { type JobRun, JobRunOutcome, JobRunSchema } from "@/gen/nama/v1/job_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type Backup,
  BackupSchema,
  type UpdateBackupSettingsRequest,
} from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fill, openApp } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { admin, fakeServer, newUser } from "@/test-server";
import type { Answers } from "@/test-server-api";
import { listFoldersOf } from "@/test-server-folders";
import { openSettings } from "@/test-settings";

const START = 1_795_000_000_000n;

afterEach(cleanup);

/**
 * Opens Settings' Backups on a Server with Backups in `/data/backups` and Upkeep disabled,
 * answering as `serverService` says.
 */
function openBackups(serverService: Answers["ServerService"]) {
  const answers = {
    ServerService: {
      getBackups: () => ({ folder: "/data/backups", settings: {}, backups: [] }),
      ...serverService,
    },
  };
  return openSettings("backups", fakeServer({ signedIn: admin }, answers).transport);
}

describe("back up now", () => {
  test("a Backup that ends before the pane looks at its Job again shows in the list", async () => {
    let backups: Backup[] = [];
    let lastRun: JobRun | undefined;
    const { transport } = fakeServer(
      { signedIn: admin },
      {
        ServerService: {
          getBackups: () => ({ folder: "/data/backups", settings: {}, backups }),
        },
        JobService: {
          listJobs: () => ({
            jobs: [{ pluginId: "upkeep", id: "back-up", name: "Back up", enabled: true, lastRun }],
          }),
          runJob: () => {
            // A small Server's Backup is written before the pane asks about the Job again.
            backups = [create(BackupSchema, { name: "nama-20261121T030000Z.tar", sizeBytes: 5n })];
            lastRun = create(JobRunSchema, {
              id: 1n,
              outcome: JobRunOutcome.SUCCEEDED,
              startedAtMs: START,
            });
            return {};
          },
        },
      },
    );
    openSettings("backups", transport);

    fireEvent.click(await screen.findByRole("button", { name: "Back up now" }));

    await expect(screen.findByText("nama-20261121T030000Z.tar")).resolves.toBeDefined();
  });
});

/** A fake Server with `backups` in `/data/backups` and Upkeep's Back up Job, last run as `lastRun` says. */
function upkeepServer(backups: Backup[], lastRun?: JobRun) {
  const { transport } = fakeServer(
    { signedIn: admin },
    {
      ServerService: { getBackups: () => ({ folder: "/data/backups", settings: {}, backups }) },
      JobService: {
        listJobs: () => ({
          jobs: [{ pluginId: "upkeep", id: "back-up", name: "Back up", enabled: true, lastRun }],
        }),
      },
    },
  );
  return transport;
}

describe("backup status and restoring", () => {
  test("restore under a Backup shows the command that restores that file from the folder", async () => {
    const written = ["nama-20261122T030000Z.tar", "nama-20261121T030000Z.tar"].map((name) =>
      create(BackupSchema, { name, sizeBytes: 5n, modifiedAtMs: START }),
    );
    openSettings("backups", upkeepServer(written));

    fireEvent.click(
      await screen.findByRole("button", { name: "Restore nama-20261121T030000Z.tar" }),
    );

    expect(
      screen.getByText("nama-server restore /data/backups/nama-20261121T030000Z.tar"),
    ).toBeDefined();
    expect(
      screen
        .getByRole("button", { name: "Close nama-20261121T030000Z.tar" })
        .getAttribute("aria-expanded"),
    ).toBe("true");
  });

  test("a failed last Backup says why at the top of the pane", async () => {
    const failed = create(JobRunSchema, {
      id: 2n,
      outcome: JobRunOutcome.FAILED,
      startedAtMs: START,
      error: "No space left on device",
    });
    openSettings("backups", upkeepServer([], failed));

    await expect(screen.findByText("No space left on device")).resolves.toBeDefined();
    expect(screen.getByRole("button", { name: "Back up now" })).toBeDefined();
  });

  test("with Upkeep disabled, the pane says the Server doesn't back itself up", async () => {
    openBackups({});

    await expect(screen.findByText("The Server doesn’t back itself up")).resolves.toBeDefined();
    expect(screen.getByRole("link", { name: "Enable Upkeep" })).toBeDefined();
    expect(screen.queryByRole("button", { name: "Back up now" })).toBeNull();
  });

  test("enable Upkeep, from a Setting search found, opens Plugins with only Upkeep", async () => {
    const router = openApp(
      "/code?settings=backups&setting=schedule",
      fakeServer(
        { signedIn: admin },
        {
          ServerService: {
            getBackups: () => ({ folder: "/data/backups", settings: {}, backups: [] }),
          },
        },
      ).transport,
    );

    fireEvent.click(await screen.findByRole("link", { name: "Enable Upkeep" }));

    await waitFor(() => {
      expect(router.state.location.href).toBe("/code?settings=plugins&plugin=upkeep");
    });
  });

  test("the Back up Job's Change link opens it in Settings' Jobs section", async () => {
    const router = openSettings("backups", upkeepServer([]));

    fireEvent.click(await screen.findByRole("link", { name: "Change" }));

    await expect(screen.findByRole("region", { name: "Back up" })).resolves.toBeDefined();
    expect(router.state.location.search).toMatchObject({ settings: "jobs", job: "upkeep:back-up" });
  });
});

describe("backup settings", () => {
  test("the Backup folder is saved once its field is left, keeping how many are kept", async () => {
    const updates: UpdateBackupSettingsRequest[] = [];
    openBackups({
      getBackups: () => ({ folder: "/data/backups", settings: { keep: 5 }, backups: [] }),
      updateBackupSettings: (request) => {
        updates.push(request);
        return {};
      },
    });

    const folder = await screen.findByLabelText("Backup folder");
    fill("Backup folder", "/mnt/backups");
    // Nothing is saved while it's typed.
    expect(updates).toStrictEqual([]);
    fireEvent.blur(folder);

    await expect
      .poll(() => updates)
      .toMatchObject([{ settings: { folder: "/mnt/backups", keep: 5 } }]);
    // Leaving the field again saves nothing more.
    fireEvent.blur(folder);
    expect(updates).toHaveLength(1);
  });

  test("backups kept left empty is saved unset, for the Server's default", async () => {
    const updates: UpdateBackupSettingsRequest[] = [];
    openBackups({
      getBackups: () => ({ folder: "/data/backups", settings: { keep: 5 }, backups: [] }),
      updateBackupSettings: (request) => {
        updates.push(request);
        return {};
      },
    });

    const kept = await screen.findByLabelText("Backups kept");
    fill("Backups kept", "");
    fireEvent.keyDown(kept, { key: "Enter" });

    await expect.poll(() => updates).toHaveLength(1);
    expect(updates[0]?.settings?.keep).toBeUndefined();
  });

  test("+ on an empty backups kept steps from its default of 7, saving 8", async () => {
    const updates: UpdateBackupSettingsRequest[] = [];
    openBackups({
      updateBackupSettings: (request) => {
        updates.push(request);
        return {};
      },
    });

    const kept = await screen.findByLabelText<HTMLInputElement>("Backups kept");
    fireEvent.click(screen.getByRole("button", { name: "Increase" }));

    await expect.poll(() => updates.at(-1)?.settings?.keep).toBe(8);
    expect(kept.value).toBe("8");
  });

  test("backups kept is held to a whole number of 1 or more by its field", async () => {
    openBackups({ updateBackupSettings: () => ({}) });

    const kept = await screen.findByLabelText<HTMLInputElement>("Backups kept");
    fill("Backups kept", "0");
    fireEvent.blur(kept);

    await waitFor(() => {
      expect(kept.value).toBe("1");
    });
  });

  test("a Backup folder the Server can't write to is said under its field", async () => {
    openBackups({
      updateBackupSettings: () => {
        throw refused(Code.InvalidArgument, "the Server can't write to /mnt/backups", {
          reason: RefusalReason.FOLDER_NOT_WRITABLE,
          field: "settings.folder",
        });
      },
    });

    const folder = await screen.findByLabelText<HTMLInputElement>("Backup folder");
    fill("Backup folder", "/mnt/backups");
    fireEvent.blur(folder);

    await expectUnderField("Backup folder", "The Server can't write to /mnt/backups.");
    // What was typed stays to be fixed.
    expect(folder.value).toBe("/mnt/backups");
  });
});

describe("where Backups are", () => {
  test("a User who manages settings but isn't an Admin has no Backups section", async () => {
    openSettings(
      "backups",
      fakeServer({ signedIn: [...newUser, Permission.MANAGE_SETTINGS] }).transport,
    );

    await screen.findByRole("heading", { name: "Playback" });

    const sections = within(screen.getByRole("navigation", { name: "Settings" }));
    expect(sections.getByRole("link", { name: "Server" })).toBeDefined();
    expect(sections.queryByRole("link", { name: "Backups" })).toBeNull();
  });
});

test("a Backup folder picked on the Server fills the field and is saved, the picker opening at the one in use", async () => {
  const updates: UpdateBackupSettingsRequest[] = [];
  openBackups({
    listServerFolders: listFoldersOf({
      "": ["/data"],
      "/data": ["/data/backups"],
      "/data/backups": [],
    }),
    updateBackupSettings: (request) => {
      updates.push(request);
      return {};
    },
  });
  fireEvent.click(await screen.findByRole("button", { name: "Pick a folder on the Server" }));
  const inUse = await screen.findByRole("treeitem", { name: "backups" });
  await waitFor(() => {
    expect(inUse.getAttribute("aria-selected")).toBe("true");
  });

  fireEvent.click(screen.getByRole("treeitem", { name: "data" }));

  await expect.poll(() => updates).toHaveLength(1);
  expect(updates[0]?.settings?.folder).toBe("/data");
  expect(screen.getByLabelText("Backup folder")).toHaveProperty("value", "/data");
});
