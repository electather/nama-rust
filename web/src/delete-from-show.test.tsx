import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  LibraryKind,
  type LibraryService,
  type MediaToDeleteSchema,
} from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { eventFeed, fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const deletesMedia = [...newUser, Permission.DELETE_MEDIA];

type Target = MessageInitShape<typeof MediaToDeleteSchema>["target"];

const TWO_EPISODES = { id: 3n, fileName: "Dark - S01E01-E02.mkv", size: 4_000_000_000n };
const SECRETS_ONLY = { id: 4n, fileName: "Dark - S01E01.mkv", size: 2_000_000_000n };
const darkId = { id: { case: "tmdb" as const, value: 70_523n } };
const secrets = {
  id: { id: { case: "tmdb" as const, value: 1_388_538n } },
  seasonNumber: 1,
  episodeNumber: 1,
  title: "Secrets",
  versions: [TWO_EPISODES, SECRETS_ONLY],
};
const lies = {
  id: { id: { case: "tmdb" as const, value: 1_388_539n } },
  seasonNumber: 1,
  episodeNumber: 2,
  title: "Lies",
  versions: [TWO_EPISODES],
};

/**
 * Dark's page in a Shows Library, for a User with `permissions`, deleting through `library` and
 * browsing through `browse` where it says.
 */
function openDark(
  permissions: Permission[],
  library: Partial<ServiceImpl<typeof LibraryService>> = {},
  browse: Partial<ServiceImpl<typeof BrowseService>> = {},
) {
  const events = eventFeed();
  const transport = fakeServer(
    {
      signedIn: permissions,
      events: events.stream,
      libraries: [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }],
      libraryService: {
        previewDeleteMedia: () => ({
          files: [TWO_EPISODES],
          bytesFreed: TWO_EPISODES.size,
          episodesTakenAlong: [{ seasonNumber: 1, episodeNumber: 1, title: "Secrets" }],
        }),
        ...library,
      },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        getShow: () => ({
          show: { id: darkId, title: "Dark" },
          seasons: [{ number: 1 }, { number: 2 }],
        }),
        listEpisodes: () => ({ episodes: [secrets, lies] }),
        listShows: () => ({ shows: [] }),
        ...browse,
      });
    },
  );
  openApp("/libraries/2/shows/tmdb-70523", transport);
  return events;
}

test("only a User with Delete media sees Delete on a Show, its Season and its Episodes", async () => {
  openDark(newUser);
  await screen.findByText(/Lies/u);
  expect(screen.queryByRole("button", { name: /^Delete/u })).toBeNull();
  cleanup();

  openDark(deletesMedia);

  expect(await screen.findByRole("button", { name: "Delete Show" })).toBeDefined();
  expect(screen.getByRole("button", { name: "Delete Season 1" })).toBeDefined();
  expect(screen.getByRole("button", { name: "Delete Lies" })).toBeDefined();
});

/** A `DeleteMedia` that records what it deletes in `deleted`. */
function recording(deleted: Target[]): Partial<ServiceImpl<typeof LibraryService>> {
  return {
    deleteMedia: ({ target }) => {
      deleted.push(target?.target);
      return {};
    },
  };
}

/** Opens the dialog of the Delete button named `name`, which asks about deleting `title`. */
async function openDialog(name: string, title: string) {
  fireEvent.click(await screen.findByRole("button", { name }));
  return screen.findByRole("alertdialog", { name: `Delete ${title}?` });
}

test("deleting an Episode in a two-Episode file names the other Episode before confirming", async () => {
  const deleted: Target[] = [];
  openDark(deletesMedia, recording(deleted));
  const dialog = await openDialog("Delete Lies", "Lies");

  expect(await within(dialog).findByText("Season 1, Episode 1: Secrets")).toBeDefined();
  expect(within(dialog).getByText("Dark - S01E01-E02.mkv")).toBeDefined();
  expect(within(dialog).getByText(/This file goes, freeing 4 GB/u)).toBeDefined();
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  await waitFor(() => {
    expect(deleted).toMatchObject([
      { case: "episode", value: { libraryId: 2n, episodeId: lies.id } },
    ]);
  });
});

test("the confirmation names the open Requests deleting cancels", async () => {
  openDark(deletesMedia, {
    previewDeleteMedia: () => ({
      files: [SECRETS_ONLY],
      bytesFreed: SECRETS_ONLY.size,
      requestsCancelled: [{ title: "Dark", seasonNumber: 1 }],
    }),
  });

  const dialog = await openDialog("Delete Lies", "Lies");

  expect(await within(dialog).findByText(/cancels this open Request/u)).toBeDefined();
  expect(within(dialog).getByText("Dark, Season 1")).toBeDefined();
});

test("deleting one Version of an Episode with two names only it", async () => {
  const deleted: Target[] = [];
  openDark(deletesMedia, recording(deleted));
  const dialog = await openDialog("Delete Secrets", "Secrets");

  fireEvent.change(within(dialog).getByLabelText("What to delete"), { target: { value: "4" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  await waitFor(() => {
    expect(deleted).toMatchObject([
      { case: "episode", value: { episodeId: secrets.id, versionId: 4n } },
    ]);
  });
});

test("Delete Season names the Season, and Delete Show the Show", async () => {
  const deleted: Target[] = [];
  openDark(deletesMedia, recording(deleted));

  const season = await openDialog("Delete Season 1", "Dark Season 1");
  fireEvent.click(within(season).getByRole("button", { name: "Delete Season 1" }));
  await waitFor(() => {
    expect(deleted).toHaveLength(1);
  });
  const show = await openDialog("Delete Show", "Dark");
  fireEvent.click(within(show).getByRole("button", { name: "Delete Show" }));

  await waitFor(() => {
    expect(deleted).toMatchObject([
      { case: "season", value: { libraryId: 2n, showId: darkId, seasonNumber: 1 } },
      { case: "show", value: { libraryId: 2n, showId: darkId } },
    ]);
  });
});

/** Sends an Event saying an Item of Dark's Library was removed. */
function removeAnItem(events: ReturnType<typeof eventFeed>) {
  events.send({ event: { case: "libraryItemRemoved", value: { libraryId: 2n, itemId: 9n } } });
}

test("an open Show page drops a deleted Episode when its Item is removed", async () => {
  const season = [secrets, lies];
  const events = openDark(newUser, {}, { listEpisodes: () => ({ episodes: season }) });
  await screen.findByText(/Lies/u);

  season.pop();
  removeAnItem(events);

  await waitFor(() => {
    expect(screen.queryByText(/Lies/u)).toBeNull();
  });
  expect(screen.getByText(/Secrets/u)).toBeDefined();
});

/** An RPC answering `answer` until `gone.now` is set, then refusing as not found. */
function until<T>(gone: { now: boolean }, answer: T) {
  return () => {
    if (gone.now) throw new ConnectError("deleted", Code.NotFound);
    return answer;
  };
}

test("an open Show page drops a Season deleted elsewhere", async () => {
  const gone = { now: false };
  const seasons = [{ number: 1 }, { number: 2 }];
  const events = openDark(
    newUser,
    {},
    {
      getShow: () => ({ show: { id: darkId, title: "Dark" }, seasons }),
      listEpisodes: until(gone, { episodes: [secrets, lies] }),
    },
  );
  await screen.findByText(/Lies/u);

  gone.now = true;
  seasons.shift();
  removeAnItem(events);

  expect(await screen.findByText("Season 1 is no longer in this Library.")).toBeDefined();
  expect(screen.queryByText(/Lies/u)).toBeNull();
});

test("an open Show page says so once the Show was deleted elsewhere", async () => {
  const gone = { now: false };
  const show = { show: { id: darkId, title: "Dark" }, seasons: [{ number: 1 }] };
  const events = openDark(newUser, {}, { getShow: until(gone, show) });
  await screen.findByText(/Lies/u);

  gone.now = true;
  removeAnItem(events);

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
});
