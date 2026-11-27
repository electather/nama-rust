import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, LibraryService, type UpdateLibraryRequest } from "@/gen/nama/v1/library_pb";
import { admin, fakeServer, fill } from "@/test-app";
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
  let library = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
  return fakeServer(
    { signedIn: admin, serverService: { listServerFolders: listFoldersOf(disk) } },
    {},
    {},
    (router) => {
      router.service(LibraryService, {
        listLibraries: () => ({ libraries: [library] }),
        getLibrary: () => ({ library }),
        updateLibrary: (request) => {
          updates.push(request);
          library = { ...library, folders: request.folders };
          return { library };
        },
      });
    },
  );
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
