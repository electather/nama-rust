import { create } from "@bufbuild/protobuf";
import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, test } from "vite-plus/test";

import { type JobRun, JobRunOutcome, JobRunSchema, JobService } from "@/gen/nama/v1/job_pb";
import { type Backup, BackupSchema } from "@/gen/nama/v1/server_pb";
import { admin, fakeServer, openApp } from "@/test-app";

const START = 1_795_000_000_000n;

describe("back up now", () => {
  test("a Backup that ends before the page looks at its Job again shows in the list", async () => {
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
            // A small Server's Backup is written before the page asks about the Job again.
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
    openApp("/admin/backups", server);

    fireEvent.click(await screen.findByRole("button", { name: "Back up now" }));

    await expect(screen.findByText("nama-20261121T030000Z.tar")).resolves.toBeDefined();
  });
});
