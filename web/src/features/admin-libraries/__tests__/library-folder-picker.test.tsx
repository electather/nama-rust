import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, type UpdateLibraryRequest } from "@/gen/nama/v1/library_pb";
import { fill } from "@/test-app";
import { admin, fakeServer } from "@/test-server";
import { listFoldersOf } from "@/test-server-folders";
import { openLibraries } from "@/test-settings";

afterEach(cleanup);

const disk = {
  "": ["/srv"],
  "/srv": ["/srv/films", "/srv/shows"],
  "/srv/films": [],
  "/srv/shows": [],
};

/** A fake Server with the Library Films in `/srv/films`, recording each change to it in `updates`. */
function libraryServer(updates: UpdateLibraryRequest[]) {
  const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
  const { server, transport } = fakeServer(
    { signedIn: admin, libraries: [films] },
    {
      ServerService: { listServerFolders: listFoldersOf(disk) },
      LibraryService: {
        updateLibrary: (request) => {
          updates.push(request);
          server.libraries = [{ ...films, folders: request.folders }];
          return { library: server.libraries[0] };
        },
      },
    },
  );
  return transport;
}

async function pick(folder: string) {
  fireEvent.click(await screen.findByRole("button", { name: "Pick a folder on the Server" }));
  fireEvent.click(await screen.findByRole("treeitem", { name: folder }));
}

test("a folder picked for a Library is saved as a new line, and one it lists already changes nothing", async () => {
  const updates: UpdateLibraryRequest[] = [];
  openLibraries(libraryServer(updates), "1");

  await pick("shows");
  await expect
    .poll(() => updates.map((update) => update.folders))
    .toStrictEqual([["/srv/films", "/srv/shows"]]);
  await pick("films");

  await expect
    .poll(() => screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Folders" }).value)
    .toBe("/srv/films\n/srv/shows");
  expect(updates).toHaveLength(1);
});

test("a folder picked for a new Library is added as a new line", async () => {
  openLibraries(libraryServer([]), "new");
  await screen.findByRole("button", { name: "Create Library" });
  fill("Folders", "/srv/films\n");

  await pick("shows");

  expect(screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Folders" }).value).toBe(
    "/srv/films\n/srv/shows",
  );
});
