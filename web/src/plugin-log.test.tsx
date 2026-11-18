import { cleanup } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";

import { openPlugin, openPluginAsAdmin, plugin } from "./test-plugins";

afterEach(cleanup);

const LOGGED_AT = Date.UTC(2026, 10, 18, 9, 30);

test("an Admin sees a Plugin's log lines newest first, with when and how much each matters", async () => {
  openPluginAsAdmin("tmdb", {
    listPluginLogLines: () => ({
      lines: [
        { atMs: BigInt(LOGGED_AT + 1000), level: PluginLogLevel.WARN, text: "TMDB said 429" },
        { atMs: BigInt(LOGGED_AT), level: PluginLogLevel.INFO, text: "fetched Heat" },
      ],
    }),
  });
  const page = await plugin("TMDB");

  const log = await page.findByRole("list", { name: "Plugin Log" });
  const lines = [...log.querySelectorAll("li")];
  expect(lines.map((line) => line.textContent)).toEqual([
    expect.stringMatching(/Warning.*TMDB said 429/u),
    expect.stringMatching(/Info.*fetched Heat/u),
  ]);
  expect(lines[1]?.querySelector("time")?.getAttribute("datetime")).toBe(
    new Date(LOGGED_AT).toISOString(),
  );
});

test("a Plugin that has logged nothing says so", async () => {
  openPluginAsAdmin("tmdb", { listPluginLogLines: () => ({ lines: [] }) });
  const page = await plugin("TMDB");

  await expect(page.findByText(/Nothing logged/u)).resolves.toBeDefined();
});

test("someone who manages settings but isn't an Admin sees no Plugin Log", async () => {
  openPlugin("tmdb");
  const page = await plugin("TMDB");

  expect(page.queryByRole("heading", { name: "Plugin Log" })).toBeNull();
});
