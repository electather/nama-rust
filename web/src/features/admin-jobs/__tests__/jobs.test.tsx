import { Code, ConnectError } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vite-plus/test";

import { type ConfigureJobRequest, JobRunOutcome, JobService } from "@/gen/nama/v1/job_pb";
import { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";
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
    logLines: [
      { atMs: START, level: PluginLogLevel.INFO, text: "forgetting Home rows" },
      {
        atMs: START + 75_000n,
        level: PluginLogLevel.ERROR,
        text: "giving up: the database was busy",
      },
    ],
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
  defaultSchedule: "0 4 * * *",
  enabled: true,
  nextRunsAtMs: [START + DAY, START + 2n * DAY, START + 3n * DAY],
  lastRun: runs[0],
};

const deleteExpired = {
  pluginId: "upkeep",
  pluginName: "Upkeep",
  id: "delete-expired",
  name: "Delete expired sign-ins",
  schedule: "10 4 * * *",
  defaultSchedule: "10 4 * * *",
  nextRunsAtMs: [START + DAY],
};

const cleanUp = {
  pluginId: "cleanup",
  pluginName: "Cleanup",
  id: "clean",
  name: "Clean up",
  schedule: "",
  enabled: true,
};

/** The Run "Run now" starts of "Delete expired sign-ins". */
const ranNow = { id: 4n, outcome: JobRunOutcome.RUNNING, startedAtMs: START, startedBy: "Ada" };

function openJobs(configured: Partial<ConfigureJobRequest>[] = []) {
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
        configureJob: ({ pluginId, jobId, enabled, schedule }) => {
          configured.push({ pluginId, jobId, enabled, schedule });
          return {};
        },
        previewJobSchedule: ({ schedule }) => {
          if (schedule.split(" ").length !== 5) {
            throw refused(Code.InvalidArgument, `the schedule "${schedule}" has 4 fields`, {
              reason: RefusalReason.SCHEDULE_INVALID,
              field: "schedule",
            });
          }
          return { nextRunsAtMs: [START, START + HOUR, START + 2n * HOUR] };
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
    expect(upkeep.getAttribute("href")).toBe("/admin/jobs?settings=plugins&plugin=upkeep");
    expect(screen.getByRole("link", { name: "Cleanup" })).toBeDefined();
    expect(screen.getByText("Every day at 04:00 Server time")).toBeDefined();
    expect(screen.getByText("Every day at 04:10 Server time")).toBeDefined();
    expect(screen.getByText("Only when an Admin runs it")).toBeDefined();
    expect(screen.getByText(/^Next run .* · Last run /u)).toBeDefined();
    expect(screen.getByText(/^Next run .* · Never run$/u)).toBeDefined();
    expect(screen.getAllByText("Off")).toHaveLength(1);
    expect(screen.getByText("Failed")).toBeDefined();
    expect(screen.queryByRole("searchbox")).toBeNull();
  });

  test("a Job's row is the target a Plugin's view in Settings links to", async () => {
    openJobs();

    const job = await screen.findByRole("button", { name: "Forget Home rows" });
    expect(job.id).toBe("upkeep/forget-home-rows");
  });

  test("a Job's sheet shows what it does, its next 3 times and each Run's detail and Plugin Log", async () => {
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
    // Only the failed Run has lines; the interrupted one says it has none.
    const lines = sheet.getByRole("list", { name: "Plugin Log" });
    expect(
      within(lines)
        .getAllByRole("listitem")
        .map((line) => line.textContent),
    ).toStrictEqual([
      expect.stringMatching(/Info.*forgetting Home rows/u),
      expect.stringMatching(/Error.*giving up: the database was busy/u),
    ]);
    expect(sheet.getByText(/^No lines kept\./u)).toBeDefined();
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

/** Opens the Configure dialog of the Job named `name`. */
async function configure(name: string) {
  fireEvent.click(await screen.findByRole("button", { name: `Configure ${name}` }));
  return within(await screen.findByRole("alertdialog", { name: `Configure ${name}` }));
}

describe("configuring a Job", () => {
  test("turns it off and gives it a schedule of its own, previewing its next times", async () => {
    const configured: Partial<ConfigureJobRequest>[] = [];
    openJobs(configured);

    const dialog = await configure("Forget Home rows");
    const on = dialog.getByRole("switch", { name: "Enabled" });
    expect(on.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(on);
    const schedule = dialog.getByLabelText("Schedule");
    expect(schedule.getAttribute("placeholder")).toBe("0 4 * * *");
    fireEvent.change(schedule, { target: { value: "0 * * * *" } });
    const next = await dialog.findByRole("list", { name: "Next runs" });
    expect(within(next).getAllByRole("listitem")).toHaveLength(3);
    fireEvent.click(dialog.getByRole("button", { name: "Save" }));

    await vi.waitFor(() => {
      expect(configured).toStrictEqual([
        { pluginId: "upkeep", jobId: "forget-home-rows", enabled: false, schedule: "0 * * * *" },
      ]);
    });
  });

  test("says why a schedule doesn't parse as it's typed", async () => {
    openJobs();

    const dialog = await configure("Forget Home rows");
    fireEvent.change(dialog.getByLabelText("Schedule"), { target: { value: "0 4 * *" } });

    await expect(dialog.findByText('The schedule "0 4 * *" has 4 fields.')).resolves.toBeDefined();
    expect(dialog.getByLabelText("Schedule").getAttribute("aria-invalid")).toBe("true");
  });

  test("reset to default saves it on, on its Plugin's schedule", async () => {
    const configured: Partial<ConfigureJobRequest>[] = [];
    openJobs(configured);

    const dialog = await configure("Delete expired sign-ins");
    expect(dialog.getByRole("switch", { name: "Enabled" }).getAttribute("aria-checked")).toBe(
      "false",
    );
    fireEvent.click(dialog.getByRole("button", { name: "Reset to default" }));
    expect(dialog.getByRole("switch", { name: "Enabled" }).getAttribute("aria-checked")).toBe(
      "true",
    );
    fireEvent.click(dialog.getByRole("button", { name: "Save" }));

    await vi.waitFor(() => {
      expect(configured).toStrictEqual([
        { pluginId: "upkeep", jobId: "delete-expired", enabled: true, schedule: "" },
      ]);
    });
  });

  test("a Job without a schedule can't be given one", async () => {
    openJobs();

    const dialog = await configure("Clean up");

    expect(dialog.getByRole("switch", { name: "Enabled" })).toBeDefined();
    expect(dialog.queryByLabelText("Schedule")).toBeNull();
    expect(dialog.getByText("It runs only when an Admin runs it.")).toBeDefined();
  });
});
