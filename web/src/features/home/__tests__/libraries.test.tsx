import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type Library = MessageInitShape<typeof LibrarySchema>;

const NEWEST = "/media/artwork/tmdb/%2Falien-backdrop.jpg";

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
