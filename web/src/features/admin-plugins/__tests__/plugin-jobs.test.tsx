import { cleanup, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { openPlugin, openPluginAsAdmin, plugin } from "@/test-plugins";

afterEach(cleanup);

const jobs = [
  { id: "forget-home-rows", name: "Forget Home rows", schedule: "0 4 * * *" },
  { id: "tidy", name: "Tidy", schedule: "" },
];

/**
 * Upkeep's page declaring `declared`, enabled unless `enabled` is false, opened by an Admin unless
 * `asAdmin` is false.
 */
async function openUpkeep(declared: typeof jobs, { asAdmin = true, enabled = true } = {}) {
  const open = asAdmin ? openPluginAsAdmin : openPlugin;
  open("upkeep", {
    getPlugin: () => ({
      plugin: { summary: { id: "upkeep", name: "Upkeep", enabled }, jobs: declared },
    }),
  });
  const page = await plugin("Upkeep");
  return page;
}

test("a Plugin's Jobs show with their schedules, each linking to it on the Jobs page", async () => {
  const page = await openUpkeep(jobs);

  const section = within(page.getByRole("region", { name: "Jobs" }));
  expect(section.getAllByRole("listitem").map((item) => item.textContent)).toStrictEqual([
    "Forget Home rowsEvery day at 04:00 Server time",
    "TidyOnly when an Admin runs it",
  ]);
  expect(section.getByRole("link", { name: "Forget Home rows" }).getAttribute("href")).toBe(
    "/admin/jobs#upkeep/forget-home-rows",
  );
});

test("a Plugin without Jobs shows no Jobs section", async () => {
  const page = await openUpkeep([]);

  expect(page.queryByRole("region", { name: "Jobs" })).toBeNull();
});

test("someone who manages settings but isn't an Admin sees no Jobs", async () => {
  const page = await openUpkeep(jobs, { asAdmin: false });

  expect(page.queryByRole("region", { name: "Jobs" })).toBeNull();
});

test("a disabled Plugin's Jobs show without links, as the Jobs page lists only enabled ones", async () => {
  const page = await openUpkeep(jobs, { enabled: false });

  const section = within(page.getByRole("region", { name: "Jobs" }));
  expect(section.getByText("Forget Home rows")).toBeDefined();
  expect(section.queryByRole("link")).toBeNull();
});
