import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { type JobRun, JobRunOutcome, JobRunSchema, JobService } from "@/gen/nama/v1/job_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type Backup,
  BackupSchema,
  type UpdateBackupSettingsRequest,
} from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type ServerState, admin, fakeServer, fill, newUser } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { openSettings } from "@/test-settings";

const START = 1_795_000_000_000n;

afterEach(cleanup);

/** A fake Server with Backups in `/data/backups` and Upkeep disabled, answering as `serverService` says. */
function backupsServer(serverService: ServerState["serverService"] = {}) {
  return fakeServer(
    {
      signedIn: admin,
      serverService: {
        getBackups: () => ({ folder: "/data/backups", settings: {}, backups: [] }),
        ...serverService,
      },
    },
    {},
    {},
    (router) => {
      router.service(JobService, { listJobs: () => ({}) });
    },
  );
}

function openBackups(serverService: ServerState["serverService"]) {
  return openSettings("backups", backupsServer(serverService));
}

describe("back up now", () => {
  test("a Backup that ends before the pane looks at its Job again shows in the list", async () => {
    let backups: Backup[] = [];
    let lastRun: JobRun | undefined;
    const server = fakeServer(
      {
        signedIn: admin,
        serverService: { getBackups: () => ({ folder: "/data/backups", settings: {}, backups }) },
      },
      {},
      {},
      (router) => {
        router.service(JobService, {
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
        });
      },
    );
    openSettings("backups", server);

    fireEvent.click(await screen.findByRole("button", { name: "Back up now" }));

    await expect(screen.findByText("nama-20261121T030000Z.tar")).resolves.toBeDefined();
  });
});

/** A fake Server with `backups` in `/data/backups` and Upkeep's Back up Job, last run as `lastRun` says. */
function upkeepServer(backups: Backup[], lastRun?: JobRun) {
  return fakeServer(
    {
      signedIn: admin,
      serverService: { getBackups: () => ({ folder: "/data/backups", settings: {}, backups }) },
    },
    {},
    {},
    (router) => {
      router.service(JobService, {
        listJobs: () => ({
          jobs: [{ pluginId: "upkeep", id: "back-up", name: "Back up", enabled: true, lastRun }],
        }),
      });
    },
  );
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
    openSettings("backups", fakeServer({ signedIn: [...newUser, Permission.MANAGE_SETTINGS] }));

    await screen.findByRole("heading", { name: "Playback" });

    const sections = within(screen.getByRole("navigation", { name: "Settings" }));
    expect(sections.getByRole("link", { name: "Server" })).toBeDefined();
    expect(sections.queryByRole("link", { name: "Backups" })).toBeNull();
  });
});
