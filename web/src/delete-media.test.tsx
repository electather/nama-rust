import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  LibraryKind,
  type LibraryService,
  type MediaToDeleteSchema,
} from "@/gen/nama/v1/library_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, newUser, openApp } from "./test-app";
import { confirmItsYou, recentAuthenticationRequired, refused } from "./test-refusal";

afterEach(cleanup);

const deletesMedia = [...newUser, Permission.DELETE_MEDIA];

const ALIEN_1080P = { id: 1n, fileName: "Alien (1979) - 1080p.mkv", size: 8_000_000_000n };
const ALIEN_2160P = { id: 2n, fileName: "Alien (1979) - 2160p.mkv", size: 60_000_000_000n };

type Target = MessageInitShape<typeof MediaToDeleteSchema>["target"];

/**
 * Alien's page in a Movies Library, for a User with `permissions`, deleting through `library`, with
 * the read-only folder that `GetMovie` says refuses deleting it, if any.
 */
function openAlien(
  permissions: Permission[],
  library: Partial<ServiceImpl<typeof LibraryService>> = {},
  readOnlyFolder?: string,
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
        getMovie: () => ({
          movie: { title: "Alien", versions: [ALIEN_2160P, ALIEN_1080P] },
          readOnlyFolder,
        }),
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
  expect(within(dialog).queryByText(/watching/u)).toBeNull();
});

test("the confirmation says how many people are watching it now", async () => {
  openAlien(deletesMedia, {
    previewDeleteMedia: () => ({ files: [ALIEN_1080P], bytesFreed: ALIEN_1080P.size, playing: 2 }),
  });

  const dialog = await openDeleteDialog();

  expect(await within(dialog).findByText(/2 people are watching it now/u)).toBeDefined();
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

test("a read-only folder disables the Delete action, saying which folder", async () => {
  openAlien(deletesMedia, {}, "/media/movies");

  const action = await screen.findByRole("button", { name: "Delete Alien" });

  expect(action).toHaveProperty("disabled", true);
  expect(action.getAttribute("aria-describedby")).not.toBeNull();
  expect(screen.getByText(/the Server can’t write to \/media\/movies/u)).toBeDefined();
});

test("a delete the Server refuses for a read-only folder says which folder", async () => {
  openAlien(deletesMedia, { deleteMedia: () => folderReadOnly("/media/movies") });
  const dialog = await openDeleteDialog();

  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  expect(
    await within(dialog).findByText(/the Server can’t write to \/media\/movies/u),
  ).toBeDefined();
});

test("the confirmation says a read-only folder refuses the delete", async () => {
  openAlien(deletesMedia, {
    previewDeleteMedia: () => ({
      files: [ALIEN_1080P],
      bytesFreed: ALIEN_1080P.size,
      readOnlyFolder: "/media/movies",
    }),
  });

  const dialog = await openDeleteDialog();

  expect(
    await within(dialog).findByText(/refuse to delete them: it can’t write to \/media\/movies/u),
  ).toBeDefined();
});

/** The Server's refusal to delete from `folder`, which it couldn't write to when it last scanned. */
function folderReadOnly(folder: string): never {
  throw refused(Code.FailedPrecondition, `media in ${folder} can't be deleted`, {
    reason: RefusalReason.FOLDER_READ_ONLY,
    params: { folder },
  });
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

test("deleting every Version when Radarr couldn't be told says on the Library it may download the Movie again", async () => {
  openAlien(deletesMedia, {
    deleteMedia: () => ({ deleted: [ALIEN_2160P, ALIEN_1080P], fulfillersNotTold: ["Radarr"] }),
  });
  const dialog = await openDeleteDialog();

  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  expect(await screen.findByRole("heading", { name: "Films" })).toBeDefined();
  expect(
    screen.getByText("Radarr couldn’t be told about the delete, so it may download Alien again."),
  ).toBeDefined();
});

test("deleting one Version when Radarr couldn't be told says so on the Movie", async () => {
  openAlien(deletesMedia, {
    deleteMedia: () => ({ deleted: [ALIEN_1080P], fulfillersNotTold: ["Radarr"] }),
  });
  const dialog = await openDeleteDialog();
  await within(dialog).findByText("Alien (1979) - 2160p.mkv");

  fireEvent.change(within(dialog).getByLabelText("What to delete"), { target: { value: "1" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  expect(
    await screen.findByText(
      "Radarr couldn’t be told about the delete, so it may download Alien again.",
    ),
  ).toBeDefined();
  expect(screen.getByRole("button", { name: "Delete Alien" })).toBeDefined();
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
