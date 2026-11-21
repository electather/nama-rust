import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";
import { openPlugin, openPluginAsAdmin, plugin } from "@/test-plugins";

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
  expect(lines.map((line) => line.textContent)).toStrictEqual([
    expect.stringMatching(/Warning.*TMDB said 429/u),
    expect.stringMatching(/Info.*fetched Heat/u),
  ]);
  expect(lines[1]?.querySelector("time")?.getAttribute("datetime")).toBe(
    new Date(LOGGED_AT).toISOString(),
  );
});

test("a level's button hides and shows that level's lines, and says how many show", async () => {
  openPluginAsAdmin("tmdb", {
    listPluginLogLines: () => ({
      lines: [
        { atMs: BigInt(LOGGED_AT + 2000), level: PluginLogLevel.ERROR, text: "TMDB timed out" },
        { atMs: BigInt(LOGGED_AT + 1000), level: PluginLogLevel.INFO, text: "fetched Ronin" },
        { atMs: BigInt(LOGGED_AT), level: PluginLogLevel.INFO, text: "fetched Heat" },
      ],
    }),
  });
  const page = await plugin("TMDB");
  const log = await page.findByRole("list", { name: "Plugin Log" });
  const levels = page.getByRole("group", { name: "Levels shown" });
  // Only the levels it logged at, each with how many lines it has.
  expect([...levels.querySelectorAll("button")].map((button) => button.textContent)).toStrictEqual([
    "Info2",
    "Error1",
  ]);
  expect(page.getByText("3 of 3 lines")).toBeDefined();

  fireEvent.click(page.getByRole("button", { name: "Info 2", pressed: true }));
  expect(log.textContent).not.toContain("fetched");
  expect(log.textContent).toContain("TMDB timed out");
  expect(page.getByText("1 of 3 lines")).toBeDefined();

  fireEvent.click(page.getByRole("button", { name: "Error 1", pressed: true }));
  expect(page.getByText("No lines at the levels shown.")).toBeDefined();

  fireEvent.click(page.getByRole("button", { name: "Info 2", pressed: false }));
  expect(log.textContent).toContain("fetched Heat");
  expect(page.getByText("2 of 3 lines")).toBeDefined();
});

test("a Plugin that has logged nothing has no Plugin Log", async () => {
  let listed = false;
  openPluginAsAdmin("tmdb", {
    listPluginLogLines: () => {
      listed = true;
      return { lines: [] };
    },
  });
  const page = await plugin("TMDB");

  await waitFor(() => {
    expect(listed).toBe(true);
  });
  expect(page.queryByRole("heading", { name: "Plugin Log" })).toBeNull();
});

test("a Plugin Log that can't be loaded says so", async () => {
  openPluginAsAdmin("tmdb", {
    listPluginLogLines: () => {
      throw new ConnectError("the database is locked", Code.Internal);
    },
  });
  const page = await plugin("TMDB");

  await expect(
    page.findByText("Couldn’t load the Plugin Log. Check that the Server is running."),
  ).resolves.toBeDefined();
});

test("someone who manages settings but isn't an Admin sees no Plugin Log", async () => {
  openPlugin("tmdb");
  const page = await plugin("TMDB");

  expect(page.queryByRole("heading", { name: "Plugin Log" })).toBeNull();
});
