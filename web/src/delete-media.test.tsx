import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  LibraryKind,
  type LibraryService,
  type MediaToDeleteSchema,
} from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, newUser, openApp } from "./test-app";
import { confirmItsYou, recentAuthenticationRequired } from "./test-refusal";

afterEach(cleanup);

const deletesMedia = [...newUser, Permission.DELETE_MEDIA];

const ALIEN_1080P = { id: 1n, fileName: "Alien (1979) - 1080p.mkv", size: 8_000_000_000n };
const ALIEN_2160P = { id: 2n, fileName: "Alien (1979) - 2160p.mkv", size: 60_000_000_000n };

type Target = MessageInitShape<typeof MediaToDeleteSchema>["target"];

/** Alien's page in a Movies Library, for a User with `permissions`, deleting through `library`. */
function openAlien(
  permissions: Permission[],
  library: Partial<ServiceImpl<typeof LibraryService>> = {},
) {
  const transport = fakeServer(
    {
      signedIn: permissions,
      libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }],
      libraryService: {
        previewDeleteMedia: ({ target }) => ({
          files: [
            ...(target?.target.case === "versionId" ? [] : [ALIEN_2160P]),
            ALIEN_1080P,
            { fileName: "Alien (1979) - 1080p.en.srt", size: 50_000n },
          ],
          bytesFreed: 68_000_050_000n,
        }),
        ...library,
      },
    },
    { reauthenticate: () => ({}) },
    {},
    (router) => {
      router.service(BrowseService, {
        getMovie: () => ({ movie: { title: "Alien", versions: [ALIEN_2160P, ALIEN_1080P] } }),
        listMovies: () => ({ movies: [] }),
      });
    },
  );
  openApp("/libraries/1/movies/tmdb-348", transport);
}

async function openDeleteDialog() {
  fireEvent.click(await screen.findByRole("button", { name: "Delete Alien" }));
  return screen.findByRole("alertdialog", { name: "Delete Alien?" });
}

test("only a User with Delete media sees the Delete action", async () => {
  openAlien(newUser);
  await screen.findByRole("link", { name: "Play" });
  expect(screen.queryByRole("button", { name: "Delete Alien" })).toBeNull();
  cleanup();

  openAlien(deletesMedia);

  expect(await screen.findByRole("button", { name: "Delete Alien" })).toBeDefined();
});

test("the confirmation lists the files that go and the space they free, and can't be undone", async () => {
  openAlien(deletesMedia);

  const dialog = await openDeleteDialog();

  expect(await within(dialog).findByText("Alien (1979) - 1080p.en.srt")).toBeDefined();
  expect(within(dialog).getByText("Alien (1979) - 2160p.mkv")).toBeDefined();
  expect(within(dialog).getByText(/These 3 files go, freeing 68 GB/u)).toBeDefined();
  expect(within(dialog).getByText(/can’t be undone/u)).toBeDefined();
});

test("deleting one Version deletes only it", async () => {
  const deleted: Target[] = [];
  openAlien(deletesMedia, {
    deleteMedia: ({ target }) => {
      deleted.push(target?.target);
      return { deleted: [ALIEN_1080P] };
    },
  });
  const dialog = await openDeleteDialog();
  await within(dialog).findByText("Alien (1979) - 2160p.mkv");

  fireEvent.change(within(dialog).getByLabelText("What to delete"), { target: { value: "1" } });
  await waitFor(() => {
    expect(within(dialog).queryByText("Alien (1979) - 2160p.mkv")).toBeNull();
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  await waitFor(() => {
    expect(deleted).toEqual([{ case: "versionId", value: 1n }]);
  });
});

test("deleting without Recent Authentication asks you to confirm it’s you first", async () => {
  const deleted: Target[] = [];
  openAlien(deletesMedia, deletingOnlyOnceConfirmed(deleted));
  const dialog = await openDeleteDialog();

  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
  await confirmItsYou();

  await waitFor(() => {
    expect(deleted.map((target) => target?.case)).toEqual(["movie"]);
  });
});

/** A `DeleteMedia` that wants Recent Authentication first, and records what it deletes then. */
function deletingOnlyOnceConfirmed(deleted: Target[]): Partial<ServiceImpl<typeof LibraryService>> {
  let asked = false;
  return {
    deleteMedia: ({ target }) => {
      if (!asked) {
        asked = true;
        throw recentAuthenticationRequired();
      }
      deleted.push(target?.target);
      return {};
    },
  };
}

test("files that couldn't be deleted are listed with why", async () => {
  openAlien(deletesMedia, {
    deleteMedia: () => ({
      deleted: [ALIEN_1080P],
      notDeleted: [{ fileName: "Alien (1979) - 2160p.mkv", reason: "Permission denied" }],
    }),
  });
  const dialog = await openDeleteDialog();

  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  expect(await screen.findByText("A file couldn’t be deleted. The rest went.")).toBeDefined();
  expect(screen.getByText("Alien (1979) - 2160p.mkv: Permission denied")).toBeDefined();
});

test("Delete media is a Permission an Admin can give", async () => {
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: admin },
      {},
      { listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }) },
    ),
  );

  fireEvent.click(await screen.findByText("Alex"));

  expect(await screen.findByRole("checkbox", { name: "Delete media" })).toHaveProperty(
    "checked",
    false,
  );
});
