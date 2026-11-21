import { Code, ConnectError } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vite-plus/test";

import { JobRunOutcome, JobService } from "@/gen/nama/v1/job_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { admin, fakeServer, openApp } from "@/test-app";
import { refused } from "@/test-refusal";

const HOUR = 3_600_000n;
const DAY = 24n * HOUR;
const START = 1_795_000_000_000n;

const runs = [
  {
    id: 3n,
    outcome: JobRunOutcome.FAILED,
    startedAtMs: START,
    endedAtMs: START + 75_000n,
    error: "the database was busy",
  },
  { id: 2n, outcome: JobRunOutcome.INTERRUPTED, startedAtMs: START - DAY, startedBy: "Ada" },
];

const forgetHomeRows = {
  pluginId: "upkeep",
  pluginName: "Upkeep",
  id: "forget-home-rows",
  name: "Forget Home rows",
  description: "Forgets the Home rows of Users who stopped visiting.",
  schedule: "0 4 * * *",
  nextRunsAtMs: [START + DAY, START + 2n * DAY, START + 3n * DAY],
  lastRun: runs[0],
};

const deleteExpired = {
  pluginId: "upkeep",
  pluginName: "Upkeep",
  id: "delete-expired",
  name: "Delete expired sign-ins",
  schedule: "10 4 * * *",
  nextRunsAtMs: [START + DAY],
};

const cleanUp = {
  pluginId: "cleanup",
  pluginName: "Cleanup",
  id: "clean",
  name: "Clean up",
  schedule: "",
};

/** The Run "Run now" starts of "Delete expired sign-ins". */
const ranNow = { id: 4n, outcome: JobRunOutcome.RUNNING, startedAtMs: START, startedBy: "Ada" };

function openJobs() {
  let ran: typeof ranNow | undefined;
  openApp(
    "/admin/jobs",
    fakeServer({ signedIn: admin }, {}, {}, (router) => {
      router.service(JobService, {
        listJobs: () => ({
          jobs: [forgetHomeRows, { ...deleteExpired, lastRun: ran }, cleanUp],
        }),
        listJobRuns: ({ jobId }) => {
          if (jobId === "clean") {
            throw new ConnectError("the database is locked", Code.Internal);
          }
          if (jobId === "delete-expired") {
            return { runs: ran ? [ran] : [] };
          }
          return { runs: jobId === "forget-home-rows" ? runs : [] };
        },
        runJob: ({ jobId }) => {
          if (jobId === "forget-home-rows") {
            throw refused(Code.FailedPrecondition, "it is running already", {
              reason: RefusalReason.JOB_RUNNING,
            });
          }
          ran = ranNow;
          return {};
        },
      });
    }),
  );
}

describe("the Jobs page", () => {
  test("lists each Plugin's Jobs under its name, with their schedule and next and last Runs", async () => {
    openJobs();

    const upkeep = await screen.findByRole("link", { name: "Upkeep" });
    expect(upkeep.getAttribute("href")).toBe("/admin/plugins/upkeep");
    expect(screen.getByRole("link", { name: "Cleanup" })).toBeDefined();
    expect(screen.getByText("Every day at 04:00 Server time")).toBeDefined();
    expect(screen.getByText("Every day at 04:10 Server time")).toBeDefined();
    expect(screen.getByText("Only when an Admin runs it")).toBeDefined();
    expect(screen.getByText(/^Next run .* · Last run /u)).toBeDefined();
    expect(screen.getByText(/^Next run .* · Never run$/u)).toBeDefined();
    expect(screen.getByText("Failed")).toBeDefined();
    expect(screen.queryByRole("searchbox")).toBeNull();
  });

  test("a Job's sheet shows what it does, its next 3 times and each Run's detail", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Forget Home rows" }));

    const sheet = within(await screen.findByRole("dialog", { name: "Forget Home rows" }));
    expect(sheet.getByText("Forgets the Home rows of Users who stopped visiting.")).toBeDefined();
    const next = sheet.getByRole("list", { name: "Next runs" });
    expect(within(next).getAllByRole("listitem")).toHaveLength(3);
    await expect(sheet.findByText("the database was busy")).resolves.toBeDefined();
    expect(sheet.getAllByText("Schedule")).toHaveLength(1);
    expect(sheet.getByText("Ada")).toBeDefined();
    expect(sheet.getByText("1 minute 15 seconds")).toBeDefined();
    expect(sheet.getByText("Interrupted")).toBeDefined();
    expect(sheet.getByText("Not known: the Server stopped during it")).toBeDefined();
  });

  test("a Job that never ran says so in its sheet", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Delete expired sign-ins" }));

    const sheet = within(await screen.findByRole("dialog", { name: "Delete expired sign-ins" }));
    await expect(sheet.findByText("It hasn’t run yet.")).resolves.toBeDefined();
  });

  test("a Job whose Runs can't be read says so in its sheet, and the list stays", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Clean up" }));

    const sheet = within(await screen.findByRole("dialog", { name: "Clean up" }));
    await expect(sheet.findByText(/^Couldn’t load its Runs/u)).resolves.toBeDefined();
    expect(screen.getByRole("link", { name: "Upkeep" })).toBeDefined();
  });

  test("run now starts a Job, and its list and sheet show the new Run without a reload", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Run Delete expired sign-ins now" }));

    await expect(screen.findByText("Running")).resolves.toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Delete expired sign-ins" }));
    const sheet = within(await screen.findByRole("dialog", { name: "Delete expired sign-ins" }));
    await expect(sheet.findByText("Running")).resolves.toBeDefined();
  });

  test("run now on a Job with a Run going says why it didn't start", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Run Forget Home rows now" }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("It’s running now. Run it again once that Run ends.");
  });
});
