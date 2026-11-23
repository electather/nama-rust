import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";
import {
  DeviceKind,
  ViewingEventSchema,
  ViewingEventService,
} from "@hub/gen/nama/hub/v1/viewing_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import {
  cleanup,
  configure,
  fireEvent,
  render,
  screen,
  waitForElementToBeRemoved,
} from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test, vi } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

const matrix = create(ViewingEventSchema, {
  id: "0197-matrix",
  externalId: "tmdb-603",
  startedAt: 1_700_000_000n,
  timeOfDayMinutes: 1260,
  stopPositionMs: 8_000_000n,
  durationMs: 8_160_000n,
  finished: true,
  deviceKind: DeviceKind.TV,
});

const pilot = create(ViewingEventSchema, {
  id: "0198-pilot",
  episode: true,
  externalId: "tvdb-349232",
  startedAt: 1_699_000_000n,
  stopPositionMs: 3_723_000n,
  durationMs: 4_500_000n,
});

/**
 * Opens `path` signed in as Ada, whose history holds `events`, or with `listFails` can't be read.
 * Ada pools until she turns it off.
 */
function open(path: string, { events = [matrix, pilot], listFails = false } = {}) {
  let history = events;
  let pooling = true;
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, { getAccount: () => ({ account: ada }) });
    service(LinkService, {
      listLinkedServers: () => ({ servers: [] }),
      listLinkedUsers: () => ({ users: [] }),
    });
    service(ViewingEventService, {
      listViewingEvents: () => {
        if (listFails) {
          throw new ConnectError("the Hub failed", Code.Internal);
        }
        return { events: history };
      },
      deleteViewingEvent: ({ id }) => {
        history = history.filter((event) => event.id !== id);
        return {};
      },
      getPooling: () => ({ pooling }),
      setPooling: (request) => {
        ({ pooling } = request);
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", path);
  render(<RouterProvider router={getRouter(transport)} />);
}

/** `file`, which should be a Blob the page exported. */
function exportedBlob(file: Blob | MediaSource | undefined) {
  if (!(file instanceof Blob)) {
    throw new TypeError("the page exported no Blob");
  }
  return file;
}

describe("viewing history", () => {
  beforeAll(() => {
    // A file's first `findBy…` waits while the route chunks load for the first time.
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  test("the Account page leads to the history", async () => {
    open("/");

    fireEvent.click(await screen.findByRole("link", { name: "See, delete or export it" }));

    await expect(screen.findByRole("heading", { name: "Viewing history" })).resolves.toBeDefined();
  });

  test("each play says what, how far and on what kind of Device", async () => {
    open("/history");

    await expect(screen.findByText("Movie tmdb-603")).resolves.toBeDefined();
    expect(screen.getByText("Finished")).toBeDefined();
    expect(screen.getByText(/ on a TV$/u)).toBeDefined();
    expect(screen.getByText("Episode tvdb-349232")).toBeDefined();
    expect(screen.getByText("Stopped at 1:02:03 of 1:15:00")).toBeDefined();
  });

  test("deleting a play takes it out of the history", async () => {
    open("/history");

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
    open("/history");

    fireEvent.click(await screen.findByRole("button", { name: "Export as JSON" }));

    expect(downloads).toStrictEqual(["nama-viewing-history.json"]);
    const [file] = exported;
    const json: unknown = JSON.parse(await exportedBlob(file).text());
    expect(json).toStrictEqual({
      events: [
        {
          id: "0197-matrix",
          externalId: "tmdb-603",
          startedAt: "1700000000",
          timeOfDayMinutes: 1260,
          stopPositionMs: "8000000",
          durationMs: "8160000",
          finished: true,
          deviceKind: "DEVICE_KIND_TV",
        },
        {
          id: "0198-pilot",
          episode: true,
          externalId: "tvdb-349232",
          startedAt: "1699000000",
          stopPositionMs: "3723000",
          durationMs: "4500000",
        },
      ],
    });
  });

  test("the Account page shows the pooling switch, on, with what turning it off can’t undo", async () => {
    open("/");

    const toggle = await screen.findByRole("switch", {
      name: "Count your finished Movies and Watchlists",
    });

    expect(toggle.getAttribute("aria-checked")).toBe("true");
    const notice = screen.getByText(/already counted can’t be taken out\.$/u);
    expect(toggle.getAttribute("aria-describedby")).toBe(notice.id);
  });

  test("turning pooling off on the history page keeps it off", async () => {
    open("/history");
    const toggle = await screen.findByRole("switch", {
      name: "Count your finished Movies and Watchlists",
    });
    await vi.waitFor(() => {
      expect(toggle.getAttribute("aria-disabled")).toBeNull();
    });

    fireEvent.click(toggle);

    await vi.waitFor(() => {
      expect(toggle.getAttribute("aria-checked")).toBe("false");
    });
  });

  test("an empty history says so, and one the Hub can't list says to reload", async () => {
    open("/history", { events: [] });
    await expect(screen.findByText("Nothing played yet.")).resolves.toBeDefined();
    cleanup();

    open("/history", { listFails: true });

    await expect(
      screen.findByText(
        "The Hub couldn’t list your history just now. Reload the page to try again.",
      ),
    ).resolves.toBeDefined();
  });
});
