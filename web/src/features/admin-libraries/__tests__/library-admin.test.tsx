import { Code, ConnectError, type ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { admin, eventFeed, fakeServer, fill, openApp } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

/** A fake Server with the Library `films`, whose Admin is signed in, with `more` services. */
function adminServer(
  events: ReadableStream<SubscribeResponse>,
  more: (router: ConnectRouter) => void,
) {
  return fakeServer({ signedIn: admin, events }, {}, {}, (router) => {
    router.service(LibraryService, { listLibraries: () => ({ libraries: [films] }) });
    more(router);
  });
}

test("a scan Event updates a Library's progress without a reload", async () => {
  const events = eventFeed();
  openApp(
    "/admin/libraries",
    adminServer(events.stream, () => {}),
  );
  await screen.findByRole("link", { name: "Films" });

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

test.each(["/admin/libraries", "/admin/libraries/1"])(
  "%s shows a Library's last scan until a scan Event replaces it",
  async (path) => {
    const events = eventFeed();
    openApp(
      path,
      adminServer(events.stream, (router) => {
        router.service(LibraryService, {
          listLibraries: () => ({ libraries: [scannedFilms] }),
          getLibrary: () => ({ library: scannedFilms }),
        });
      }),
    );
    const stored = /^Last scanned .*2026.*: 3 new, 1 changed and 2 gone$/u;
    await expect(screen.findByText(stored)).resolves.toBeDefined();

    events.send({ event: { case: "scanStarted", value: { libraryId: 1n } } });
    await expect(screen.findByText("Scanning…")).resolves.toBeDefined();
    expect(screen.queryByText(stored)).toBeNull();
  },
);

test("a Library that has never finished a scan says no scan was seen", async () => {
  openApp(
    "/admin/libraries/1",
    adminServer(new ReadableStream(), (router) => {
      router.service(LibraryService, { getLibrary: () => ({ library: films }) });
    }),
  );

  await expect(screen.findByText("No scan seen yet.")).resolves.toBeDefined();
});

test("a Library that isn't on the Server shows the not-found page, under the app chrome", async () => {
  openApp(
    "/admin/libraries/9",
    adminServer(new ReadableStream(), (router) => {
      router.service(LibraryService, {
        getLibrary: () => {
          throw new ConnectError("there is no Library 9", Code.NotFound);
        },
      });
    }),
  );

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
  expect(screen.getByRole("navigation", { name: "Admin" })).toBeDefined();
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

describe.each([
  ["/admin/libraries", "Create Library"],
  ["/admin/libraries/1", "Save changes"],
])("on %s", (path, action) => {
  // Each message is already the sentence shown, which the form shows as it is.
  test.each([
    ["Name", Code.InvalidArgument, { field: "name" }, "A Library's name can't be blank."],
    ["Name", Code.AlreadyExists, nameTaken, 'The Library "Films" already has this name.'],
    ["Folders", Code.InvalidArgument, { field: "folders[2]" }, "The folder srv is not absolute."],
    // oxlint-disable-next-line max-params -- test.each hands a row's columns over as parameters
  ] as const)("a refused %s shows under its field", async (label, code, refusal, shown) => {
    openApp(path, serverRefusingLibraryChanges(refused(code, shown, refusal)));
    await screen.findByRole("button", { name: action });
    fill("Name", "Films");
    fireEvent.change(screen.getByRole("textbox", { name: "Folders" }), {
      target: { value: "/srv/films" },
    });
    fireEvent.click(screen.getByRole("button", { name: action }));

    await expectUnderField(label, shown);
  });
});
