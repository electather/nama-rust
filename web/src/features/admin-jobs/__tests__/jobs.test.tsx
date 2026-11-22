import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vite-plus/test";

import { type ConfigureJobRequest, JobRunOutcome, JobService } from "@/gen/nama/v1/job_pb";
import { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, openApp } from "@/test-app";
import { refused } from "@/test-refusal";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

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

/**
 * Opens the Jobs section of Settings, at the Job `?job=` names if `job` is given, on a fake Server
 * that keeps each configuring in `configured` and in its Job.
 */
function openJobs(job?: string, configured: Partial<ConfigureJobRequest>[] = []) {
  let ran: typeof ranNow | undefined;
  const jobs = [{ ...forgetHomeRows }, { ...deleteExpired }, { ...cleanUp }];
  return openApp(
    `/code?settings=jobs${job === undefined ? "" : `&job=${job}`}`,
    fakeServer({ signedIn: admin }, {}, {}, (router) => {
      router.service(JobService, {
        listJobs: () => ({
          jobs: jobs.map((each) =>
            each.id === "delete-expired" ? { ...each, lastRun: ran } : each,
          ),
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
          if (schedule.split(" ").length === 4) {
            throw refused(Code.InvalidArgument, `the schedule "${schedule}" has 4 fields`, {
              reason: RefusalReason.SCHEDULE_INVALID,
              field: "schedule",
            });
          }
          configured.push({ pluginId, jobId, enabled, schedule });
          const saved = jobs.find((each) => each.id === jobId);
          if (saved) {
            Object.assign(saved, {
              enabled,
              schedule: schedule === "" ? saved.schedule : schedule,
            });
          }
          return {};
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

describe("the Jobs section", () => {
  test("lists each Plugin's Jobs under its name, with their schedule and next and last Runs", async () => {
    openJobs();

    const upkeep = within(await screen.findByRole("list", { name: "Upkeep" }));
    expect(upkeep.getAllByRole("link").map((link) => link.textContent)).toStrictEqual([
      "Forget Home rows",
      "Delete expired sign-ins",
    ]);
    expect(screen.getByRole("list", { name: "Cleanup" })).toBeDefined();
    expect(
      upkeep.getByText(/^Every day at 04:00 Server time · Next run .* · Last run /u),
    ).toBeDefined();
    expect(
      upkeep.getByText(/^Every day at 04:10 Server time · Next run .* · Never run$/u),
    ).toBeDefined();
    expect(screen.getByText("Only when an Admin runs it · Never run")).toBeDefined();
    expect(screen.getAllByText("Off")).toHaveLength(1);
    expect(screen.getByText("Failed")).toBeDefined();
  });

  test("a Job opens in place of the list with what it does, its next 3 times and each Run's detail and Plugin Log", async () => {
    const router = openJobs();

    fireEvent.click(await screen.findByRole("link", { name: "Forget Home rows" }));

    const job = within(await screen.findByRole("region", { name: "Forget Home rows" }));
    expect(router.state.location.search).toMatchObject({
      settings: "jobs",
      job: "upkeep:forget-home-rows",
    });
    expect(screen.queryByRole("list", { name: "Upkeep" })).toBeNull();
    expect(job.getByText("Forgets the Home rows of Users who stopped visiting.")).toBeDefined();
    const next = job.getByRole("list", { name: "Next runs" });
    expect(within(next).getAllByRole("listitem")).toHaveLength(3);
    await expect(job.findByText("the database was busy")).resolves.toBeDefined();
    expect(job.getByText("Ada")).toBeDefined();
    expect(job.getByText("1 minute 15 seconds")).toBeDefined();
    expect(job.getByText("Interrupted")).toBeDefined();
    expect(job.getByText("Not known: the Server stopped during it")).toBeDefined();
    // Only the failed Run has lines; the interrupted one says it has none.
    const lines = job.getByRole("list", { name: "Plugin Log" });
    expect(
      within(lines)
        .getAllByRole("listitem")
        .map((line) => line.textContent),
    ).toStrictEqual([
      expect.stringMatching(/Info.*forgetting Home rows/u),
      expect.stringMatching(/Error.*giving up: the database was busy/u),
    ]);
    expect(job.getByText(/^No lines kept\./u)).toBeDefined();
  });

  test("an open Job's crumbs are Jobs › its name, and Jobs goes back to the list", async () => {
    const router = openJobs("upkeep:forget-home-rows");

    await screen.findByRole("region", { name: "Forget Home rows" });
    const trail = within(screen.getByRole("navigation", { name: "Breadcrumbs" }));
    expect(trail.getByText("Forget Home rows")).toBeDefined();
    fireEvent.click(trail.getByRole("link", { name: "Jobs" }));

    await expect(screen.findByRole("list", { name: "Upkeep" })).resolves.toBeDefined();
    expect(router.state.location.search.job).toBeUndefined();
  });

  test("an id no Job has shows the list", async () => {
    openJobs("upkeep:gone");

    await expect(screen.findByRole("list", { name: "Upkeep" })).resolves.toBeDefined();
  });

  test("a Job that never ran says so", async () => {
    openJobs("upkeep:delete-expired");

    await expect(screen.findByText("It hasn’t run yet.")).resolves.toBeDefined();
  });

  test("a Job whose Runs can't be read says so, and the rest of it stays", async () => {
    openJobs("cleanup:clean");

    await expect(screen.findByText(/^Couldn’t load its Runs/u)).resolves.toBeDefined();
    expect(screen.getByRole("region", { name: "Clean up" })).toBeDefined();
  });

  test("run now starts a Job, and its row and Runs show the new Run without a reload", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Run Delete expired sign-ins now" }));

    await expect(screen.findByText("Running")).resolves.toBeDefined();
    fireEvent.click(screen.getByRole("link", { name: "Delete expired sign-ins" }));
    const job = within(await screen.findByRole("region", { name: "Delete expired sign-ins" }));
    await expect(job.findByText("Running")).resolves.toBeDefined();
  });

  test("run now on a Job with a Run going says why it didn't start", async () => {
    openJobs();

    fireEvent.click(await screen.findByRole("button", { name: "Run Forget Home rows now" }));

    const alert = await screen.findByText("It’s running now. Run it again once that Run ends.");
    expect(alert.getAttribute("role")).toBe("alert");
  });
});

describe("configuring a Job", () => {
  test("turning it off saves at once, and a schedule of its own once the field is left", async () => {
    const configured: Partial<ConfigureJobRequest>[] = [];
    openJobs("upkeep:forget-home-rows", configured);

    const on = await screen.findByRole("switch", { name: "Enabled" });
    expect(on.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(on);
    await vi.waitFor(() => {
      expect(configured).toHaveLength(1);
    });
    const schedule = screen.getByLabelText("Schedule");
    expect(schedule.getAttribute("placeholder")).toBe("0 4 * * *");
    fireEvent.change(schedule, { target: { value: "0 * * * *" } });
    fireEvent.blur(schedule);

    await vi.waitFor(() => {
      expect(configured).toStrictEqual([
        { pluginId: "upkeep", jobId: "forget-home-rows", enabled: false, schedule: "" },
        { pluginId: "upkeep", jobId: "forget-home-rows", enabled: false, schedule: "0 * * * *" },
      ]);
    });
    expect(screen.getByRole("switch", { name: "Enabled" }).getAttribute("aria-checked")).toBe(
      "false",
    );
  });

  test("says why the Server can't read a schedule, under it", async () => {
    openJobs("upkeep:forget-home-rows");

    const schedule = await screen.findByLabelText("Schedule");
    fireEvent.change(schedule, { target: { value: "0 4 * *" } });
    fireEvent.blur(schedule);

    await expect(screen.findByText('The schedule "0 4 * *" has 4 fields.')).resolves.toBeDefined();
    expect(screen.getByLabelText("Schedule").getAttribute("aria-invalid")).toBe("true");
  });

  test("a Job without a schedule can't be given one", async () => {
    openJobs("cleanup:clean");

    await expect(screen.findByRole("switch", { name: "Enabled" })).resolves.toBeDefined();
    expect(screen.queryByLabelText("Schedule")).toBeNull();
    expect(screen.getByText("It runs only when an Admin runs it.")).toBeDefined();
  });
});

describe("where Jobs is", () => {
  test("a User who manages Libraries but isn't an Admin has no Jobs section", async () => {
    openSettings("jobs", fakeServer({ signedIn: [Permission.MANAGE_LIBRARIES] }));

    await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
    expect(screen.queryByRole("link", { name: "Jobs" })).toBeNull();
  });

  test("it's under Activity, after Now Playing, and searching Settings for tasks finds it", async () => {
    openSettings("playback", fakeServer({ signedIn: admin }));

    await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
    const nav = within(screen.getByRole("navigation", { name: "Settings" }));
    const links = nav.getAllByRole("link").map((link) => link.textContent);
    expect(links.slice(links.indexOf("Now Playing"), links.indexOf("Server") + 1)).toStrictEqual([
      "Now Playing",
      "Jobs",
      "Server",
    ]);
    fireEvent.change(screen.getByRole("combobox", { name: "Search settings" }), {
      target: { value: "tasks" },
    });

    expect(screen.getAllByRole("option").map((option) => option.textContent)).toStrictEqual([
      "Jobs",
    ]);
  });
});
