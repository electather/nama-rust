import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, newUser, openApp } from "./test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type Library = MessageInitShape<typeof LibrarySchema>;

const NEWEST = "/media/artwork/tmdb/%2Falien-backdrop.jpg";
const CUSTOM = "/media/custom-images/library/1/backdrop/v1";

/** The app at `path` for a User with `permissions`, on a Server whose Libraries are `libraries()` now. */
function open(path: string, permissions: Permission[], libraries: () => Library[]) {
  const transport = fakeServer(
    {
      signedIn: permissions,
      libraryService: {
        listLibraries: () => ({ libraries: libraries() }),
        getLibrary: () => ({ library: libraries()[0] }),
      },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, { listContinueWatching: () => ({}) });
    },
  );
  openApp(path, transport);
}

test("the home page shows a Library by its Backdrop, and one without by a placeholder", async () => {
  open("/", newUser, () => [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES, backdropUrl: NEWEST },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ]);

  const films = await screen.findByRole("link", { name: /Films/u });
  const tv = screen.getByRole("link", { name: /TV/u });

  expect(films.getAttribute("href")).toBe("/libraries/1");
  expect(films.querySelector("img")?.getAttribute("src")).toBe(NEWEST);
  expect(within(films).getByText("Movies")).toBeDefined();
  expect(tv.querySelector("img")).toBeNull();
  expect(within(tv).getByText("Shows")).toBeDefined();
  expect(screen.queryByLabelText(/Upload/u)).toBeNull();
});

test("an uploaded Backdrop goes to the Library's path and shows without a reload", async () => {
  let library: Library = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, backdropUrl: NEWEST };
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(() => {
    library = { ...library, backdropUrl: CUSTOM, customBackdrop: true };
    return Promise.resolve(new Response(null, { status: 204 }));
  });
  open("/admin/libraries/1", admin, () => [library]);
  const backdrop = await screen.findByRole("img", { name: "Backdrop" });
  expect(backdrop.getAttribute("src")).toBe(NEWEST);
  expect(screen.queryByRole("button", { name: /Remove custom image/u })).toBeNull();

  const file = new File(["backdrop"], "backdrop.jpg", { type: "image/jpeg" });
  fireEvent.change(screen.getByLabelText("Upload a Backdrop"), { target: { files: [file] } });

  await waitFor(() => {
    expect(screen.getByRole("img", { name: "Backdrop" }).getAttribute("src")).toBe(CUSTOM);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/library/1/backdrop", {
    method: "PUT",
    body: file,
  });
});

test("removing a Library's Custom Image brings the newest title's Backdrop back", async () => {
  let library: Library = {
    id: 1n,
    name: "Films",
    kind: LibraryKind.MOVIES,
    backdropUrl: CUSTOM,
    customBackdrop: true,
  };
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(() => {
    library = { ...library, backdropUrl: NEWEST, customBackdrop: false };
    return Promise.resolve(new Response(null, { status: 204 }));
  });
  open("/admin/libraries/1", [Permission.MANAGE_LIBRARIES], () => [library]);

  fireEvent.click(await screen.findByRole("button", { name: "Remove custom image: Backdrop" }));

  await waitFor(() => {
    expect(screen.getByRole("img", { name: "Backdrop" }).getAttribute("src")).toBe(NEWEST);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/library/1/backdrop", { method: "DELETE" });
});
