import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  ExtensionPoint,
  type ExtensionPointFailureSchema,
  PluginKind,
} from "@/gen/nama/v1/plugin_pb";

import { openPlugin, plugin } from "./test-plugins";

afterEach(cleanup);

const FAILED_AT = Date.UTC(2026, 10, 16, 9, 30);

type Failure = MessageInitShape<typeof ExtensionPointFailureSchema>;

/** The page of an enabled Recogniser and Notifier Plugin, whose points' last calls failed as `failures` say. */
function openFlaky(failures: Failure[] = []) {
  openPlugin("anime-numbers", {
    getPlugin: () => ({
      plugin: {
        summary: {
          id: "anime-numbers",
          name: "Anime numbers",
          version: "0.1.0",
          kind: PluginKind.INSTALLED,
          enabled: true,
          extensionPoints: [ExtensionPoint.RECOGNISER, ExtensionPoint.NOTIFIER],
        },
        failures,
      },
    }),
  });
  return plugin("Anime numbers");
}

function failed(point: ExtensionPoint, lastError: string, failedInARow: number): Failure {
  return { point, lastError, atMs: BigInt(FAILED_AT), failedInARow };
}

test("a Recogniser whose calls keep failing shows its last error, when and how often", async () => {
  const page = await openFlaky([failed(ExtensionPoint.RECOGNISER, "the anime list timed out", 3)]);

  const failing = page.getByRole("region", { name: "Recogniser failing" });
  expect(failing.textContent).toContain("Its last 3 calls failed");
  expect(failing.textContent).toContain("the anime list timed out");
  expect(failing.textContent).toContain("stay undecided and are tried again at the next Scan");
  expect(failing.querySelector("time")?.getAttribute("datetime")).toBe(
    new Date(FAILED_AT).toISOString(),
  );
});

test("a Recogniser whose one call failed says so in the singular", async () => {
  const page = await openFlaky([failed(ExtensionPoint.RECOGNISER, "bad answer", 1)]);

  const failing = page.getByRole("region", { name: "Recogniser failing" });
  expect(failing.textContent).toContain("Its last call failed");
});

test("each failing Extension Point shows its own failure", async () => {
  const page = await openFlaky([
    failed(ExtensionPoint.RECOGNISER, "the anime list timed out", 2),
    failed(ExtensionPoint.NOTIFIER, "the topic is gone", 1),
  ]);

  const notifier = page.getByRole("region", { name: "Notifier failing" });
  expect(notifier.textContent).toContain("Its last call failed");
  expect(notifier.textContent).toContain("the topic is gone");
  expect(notifier.textContent).not.toContain("Scan");
  expect(page.getByRole("region", { name: "Recogniser failing" }).textContent).toContain(
    "the anime list timed out",
  );
});

test("a Plugin whose calls answer shows no failure", async () => {
  const page = await openFlaky();

  expect(page.queryByRole("region", { name: /failing/u })).toBeNull();
});
