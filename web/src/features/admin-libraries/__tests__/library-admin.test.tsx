import { Code, type ConnectError, type ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { admin, eventFeed, fakeServer, fill } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { findSettingsLink, openLibraries } from "@/test-settings";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

/** A fake Server with the Library `films`, whose Admin is signed in, with `more` services. */
function adminServer(
  events: ReadableStream<SubscribeResponse>,
  more: (router: ConnectRouter) => void,
) {
  return fakeServer({ signedIn: admin, events }, {}, {}, (router) => {
    router.service(LibraryService, {
      listLibraries: () => ({ libraries: [films] }),
      getLibrary: () => ({ library: films }),
    });
    more(router);
  });
}

test("a scan Event updates a Library's progress on the list without a reload", async () => {
  const events = eventFeed();
  openLibraries(adminServer(events.stream, () => {}));
  await findSettingsLink("Films");

  events.send({
    event: { case: "scanProgress", value: { libraryId: 1n, filesDone: 3n, filesTotal: 10n } },
  });
  await expect(screen.findByText("Scanning: 3 of 10 files")).resolves.toBeDefined();

  events.send({
    event: { case: "scanFinished", value: { libraryId: 1n, added: 9n, updated: 0n, removed: 1n } },
  });
  await expect(screen.findByText("Scanned: 9 new, 0 changed and 1 gone")).resolves.toBeDefined();
});

/** `films`, last scanned on 18 Nov 2026 at 14:02 UTC. */
const scannedFilms = {
  ...films,
  lastScan: { finishedAt: 1_795_010_520n, added: 3n, updated: 1n, removed: 2n, unchanged: 50n },
};

/** A fake Server with only `scannedFilms`, fed `events`. */
function scannedServer(events: ReadableStream<SubscribeResponse>) {
  return adminServer(events, (router) => {
    router.service(LibraryService, {
      listLibraries: () => ({ libraries: [scannedFilms] }),
      getLibrary: () => ({ library: scannedFilms }),
    });
  });
}

test("a Library's row says what it holds, its folders and when it was last scanned", async () => {
  openLibraries(scannedServer(new ReadableStream()));

  const link = await findSettingsLink("Films");
  expect(link.closest("li")?.textContent).toMatch(/^FilmsMovies · 1 folder · Last scanned .*2026/u);
  expect(screen.getByRole("button", { name: "Scan Films" })).toBeDefined();
});

test("a Library shows its last scan until a scan Event replaces it", async () => {
  const events = eventFeed();
  openLibraries(scannedServer(events.stream), "1");
  const stored = /^Last scanned .*2026.*: 3 new, 1 changed and 2 gone$/u;
  await expect(screen.findByText(stored)).resolves.toBeDefined();

  events.send({ event: { case: "scanStarted", value: { libraryId: 1n } } });
  await expect(screen.findByText("Scanning…")).resolves.toBeDefined();
  expect(screen.queryByText(stored)).toBeNull();
});

test("a Library that has never finished a scan says no scan was seen", async () => {
  openLibraries(
    adminServer(new ReadableStream(), () => {}),
    "1",
  );

  await expect(screen.findByText("No scan seen yet.")).resolves.toBeDefined();
});

test("an id no Library has shows the list", async () => {
  openLibraries(
    adminServer(new ReadableStream(), () => {}),
    "9",
  );

  await expect(findSettingsLink("Films")).resolves.toBeDefined();
  const crumbs = screen.getByRole("navigation", { name: "Breadcrumbs" });
  expect(crumbs.textContent).toBe("Libraries");
});

/** A fake Server whose Library changes are refused with `error`. */
function serverRefusingLibraryChanges(error: ConnectError) {
  const refuse = () => {
    throw error;
  };
  return adminServer(new ReadableStream(), (router) => {
    router.service(LibraryService, {
      listLibraries: () => ({ libraries: [films] }),
      getLibrary: () => ({ library: films }),
      createLibrary: refuse,
      updateLibrary: refuse,
    });
  });
}

const nameTaken = { reason: RefusalReason.LIBRARY_NAME_TAKEN, field: "name" };

// Each message is already the sentence shown, which the form shows as it is.
const refusals = [
  ["Name", Code.InvalidArgument, { field: "name" }, "A Library's name can't be blank."],
  ["Name", Code.AlreadyExists, nameTaken, 'The Library "Films" already has this name.'],
  ["Folders", Code.InvalidArgument, { field: "folders[2]" }, "The folder srv is not absolute."],
] as const;

describe("a new Library", () => {
  test.each(refusals)(
    "refused for its %s says so under the field",
    // oxlint-disable-next-line max-params -- test.each hands a row's columns over as parameters
    async (label, code, refusal, shown) => {
      openLibraries(serverRefusingLibraryChanges(refused(code, shown, refusal)), "new");
      await screen.findByRole("button", { name: "Create Library" });
      fill("Name", "Films");
      fill("Folders", "/srv/films");
      fireEvent.click(screen.getByRole("button", { name: "Create Library" }));

      await expectUnderField(label, shown);
    },
  );
});

describe("a Library's change", () => {
  test.each(refusals)(
    "refused for its %s says so in the toast",
    // oxlint-disable-next-line max-params -- test.each hands a row's columns over as parameters
    async (label, code, refusal, shown) => {
      openLibraries(serverRefusingLibraryChanges(refused(code, shown, refusal)), "1");
      const field = await screen.findByRole("textbox", { name: label });
      fireEvent.change(field, { target: { value: "srv" } });
      fireEvent.blur(field);

      await expect(screen.findByText(shown)).resolves.toBeDefined();
      expect(screen.getByText("Not saved")).toBeDefined();
    },
  );
});

test("a refused change to a Library's folders puts back the saved ones", async () => {
  const shown = "The folder srv is not absolute.";
  openLibraries(
    serverRefusingLibraryChanges(refused(Code.InvalidArgument, shown, { field: "folders[0]" })),
    "1",
  );
  const folders = await screen.findByRole("textbox", { name: "Folders" });
  fireEvent.change(folders, { target: { value: "srv" } });
  fireEvent.blur(folders);

  await screen.findByText(shown);
  expect(screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Folders" }).value).toBe(
    films.folders.join("\n"),
  );
});
