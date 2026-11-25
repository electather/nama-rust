import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitForElementToBeRemoved } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { DeviceKind, ViewingEventSchema } from "@/gen/nama/hub/v1/viewing_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const matrix = create(ViewingEventSchema, {
  id: "e1",
  externalId: "tmdb-603",
  startedAt: 1_700_000_000n,
  finished: true,
  deviceKind: DeviceKind.TV,
});
const episode = create(ViewingEventSchema, {
  id: "e2",
  episode: true,
  externalId: "tvdb-349232",
  startedAt: 1_699_000_000n,
  stopPositionMs: 3_723_000n,
  durationMs: 4_500_000n,
});

/** The file the page exported. */
function exportedBlob(file: Blob | MediaSource | undefined) {
  if (!(file instanceof Blob)) {
    throw new TypeError("the page exported no Blob");
  }
  return file;
}

function openHistory(events = [matrix, episode]) {
  const fake = accountHub({ events });
  openApp("/?settings=account-history", fake.transport);
  return fake.hub;
}

test("each play says what, how far and on what kind of Device", async () => {
  openHistory();

  await expect(screen.findByText("Movie tmdb-603")).resolves.toBeDefined();
  expect(screen.getByText(/ on a TV · Finished$/u)).toBeDefined();
  expect(screen.getByText("Episode tvdb-349232")).toBeDefined();
  expect(screen.getByText(/Stopped at 1:02:03 of 1:15:00$/u)).toBeDefined();
});

test("deleting a play takes it out of the history", async () => {
  openHistory();

  fireEvent.click(await screen.findByRole("button", { name: /^Delete Movie tmdb-603, /u }));

  await waitForElementToBeRemoved(() => screen.queryByText("Movie tmdb-603"));
  expect(screen.getByText("Episode tvdb-349232")).toBeDefined();
});

test("exporting downloads every play as JSON", async () => {
  const exported: (Blob | MediaSource)[] = [];
  vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
    exported.push(blob);
    return "blob:history";
  });
  const downloads: string[] = [];
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function record(
    this: HTMLAnchorElement,
  ) {
    downloads.push(this.download);
  });
  openHistory([matrix]);

  fireEvent.click(await screen.findByRole("button", { name: "Export as JSON" }));

  expect(downloads).toStrictEqual(["nama-viewing-history.json"]);
  const json: unknown = JSON.parse(await exportedBlob(exported[0]).text());
  expect(json).toStrictEqual({
    events: [
      {
        id: "e1",
        externalId: "tmdb-603",
        startedAt: "1700000000",
        finished: true,
        deviceKind: "DEVICE_KIND_TV",
      },
    ],
  });
});

test("turning pooling off keeps it off", async () => {
  const hub = openHistory();

  const pooling = await screen.findByRole("switch", {
    name: "Count your finished Movies and Watchlists",
  });
  expect(pooling.getAttribute("aria-checked")).toBe("true");
  expect(screen.getByText(/the Movies already counted can’t be taken out/u)).toBeDefined();
  fireEvent.click(pooling);

  await vi.waitFor(() => {
    expect(pooling.getAttribute("aria-checked")).toBe("false");
  });
  expect(hub.pooling).toBe(false);
});

test("an empty history says nothing has played", async () => {
  openHistory([]);

  await expect(screen.findByText("Nothing played yet.")).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Export as JSON" })).toBeNull();
});
