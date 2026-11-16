import { cleanup } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ExtensionPoint, PluginKind } from "@/gen/nama/v1/plugin_pb";

import { openPlugin, plugin } from "./test-plugins";

afterEach(cleanup);

const FAILED_AT = Date.UTC(2026, 10, 16, 9, 30);

/** The page of an enabled Recogniser Plugin, whose last calls failed as `failure` says. */
function openRecogniser(failure?: { lastError: string; atMs: bigint; failedInARow: number }) {
  openPlugin("anime-numbers", {
    getPlugin: () => ({
      plugin: {
        summary: {
          id: "anime-numbers",
          name: "Anime numbers",
          version: "0.1.0",
          kind: PluginKind.INSTALLED,
          enabled: true,
          extensionPoints: [ExtensionPoint.RECOGNISER],
        },
        recogniserFailure: failure,
      },
    }),
  });
  return plugin("Anime numbers");
}

test("a Recogniser whose calls keep failing shows its last error, when and how often", async () => {
  const page = await openRecogniser({
    lastError: "the anime list timed out",
    atMs: BigInt(FAILED_AT),
    failedInARow: 3,
  });

  const failing = page.getByRole("region", { name: "Recogniser failing" });
  expect(failing.textContent).toContain("Its last 3 calls failed");
  expect(failing.textContent).toContain("the anime list timed out");
  expect(failing.textContent).toContain("stay undecided and are tried again at the next Scan");
  expect(failing.querySelector("time")?.getAttribute("datetime")).toBe(
    new Date(FAILED_AT).toISOString(),
  );
});

test("a Recogniser whose one call failed says so in the singular", async () => {
  const page = await openRecogniser({
    lastError: "bad answer",
    atMs: BigInt(FAILED_AT),
    failedInARow: 1,
  });

  const failing = page.getByRole("region", { name: "Recogniser failing" });
  expect(failing.textContent).toContain("Its last call failed");
});

test("a Recogniser that answers shows no failure", async () => {
  const page = await openRecogniser();

  expect(page.queryByRole("region", { name: "Recogniser failing" })).toBeNull();
});
