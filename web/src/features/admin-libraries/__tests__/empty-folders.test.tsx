import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { admin, eventFeed, fakeServer } from "@/test-app";
import { openLibraries } from "@/test-settings";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

test("removing a folder's missing items scans it so, and its warning goes once the scan finishes", async () => {
  const events = eventFeed();
  let library = { ...films, emptyFolders: ["/srv/films"] };
  const removedIn: (string | undefined)[] = [];
  openLibraries(
    fakeServer({ signedIn: admin, events: events.stream }, {}, {}, (router) => {
      router.service(LibraryService, {
        listLibraries: () => ({ libraries: [library] }),
        getLibrary: () => ({ library }),
        scanLibrary: ({ removeMissingIn }) => {
          removedIn.push(removeMissingIn);
          library = { ...films, emptyFolders: [] };
          return {};
        },
      });
    }),
    "1",
  );
  const warning = await screen.findByText(/looks empty/u);
  expect(warning.textContent).toBe(
    "/srv/films looks empty. Is the drive connected? Nothing was removed.",
  );

  fireEvent.click(screen.getByRole("button", { name: "Remove missing items" }));
  await waitFor(() => {
    expect(removedIn).toStrictEqual(["/srv/films"]);
  });
  events.send({ event: { case: "scanFinished", value: { libraryId: 1n, removed: 1n } } });

  await waitFor(() => {
    expect(screen.queryByText(/looks empty/u)).toBeNull();
  });
});
