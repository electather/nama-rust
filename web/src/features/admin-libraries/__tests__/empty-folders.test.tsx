import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { eventFeed } from "@/test-app";
import { admin, fakeServer } from "@/test-server";
import { openLibraries } from "@/test-settings";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

test("removing a folder's missing items scans it so, and its warning goes once the scan finishes", async () => {
  const events = eventFeed();
  const removedIn: (string | undefined)[] = [];
  const { server, transport } = fakeServer(
    {
      signedIn: admin,
      events: events.stream,
      libraries: [{ ...films, emptyFolders: ["/srv/films"] }],
    },
    {
      LibraryService: {
        scanLibrary: ({ removeMissingIn }) => {
          removedIn.push(removeMissingIn);
          server.libraries = [{ ...films, emptyFolders: [] }];
          return {};
        },
      },
    },
  );
  openLibraries(transport, "1");
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
