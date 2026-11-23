import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { type ClearMatchRequestSchema, MatchService, TitleKind } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { layOutShelves } from "@/test-layout";
import { openMenu, pickFromMenu } from "@/test-menu";

// A Show's row of Episodes is virtual, so it needs a layout to show its cards.
beforeEach(layOutShelves);
afterEach(cleanup);

const managesLibraries = [...newUser, Permission.MANAGE_LIBRARIES];
const tmdb = (value: string) => ({ namespace: "tmdb", value });
const POSTER = "/media/artwork/tmdb/%2Fheat.jpg";
/** The name of the title with the TMDB id `id`, as the fake Server has it. */
function nameOf(id: { value: string } | undefined) {
  return { "348": "Alien", "949": "Heat", "70523": "Dark" }[id?.value ?? ""] ?? "";
}

/** What the fake Server was asked to Match or Unmatch. */
interface Asked {
  searched?: unknown;
  matched?: unknown;
  cleared?: MessageInitShape<typeof ClearMatchRequestSchema>;
}

/** The fake Server's Match service, recording into `asked` what it was asked. */
function matchService(asked: Asked): Partial<ServiceImpl<typeof MatchService>> {
  return {
    searchTitles: (request) => {
      asked.searched = { ...request };
      return {
        candidates: [{ id: tmdb("949"), title: "Heat", year: 1995, posterUrl: POSTER }],
      };
    },
    setMatch: ({ matchedTitle, titleId }) => {
      asked.matched = { matchedTitle, titleId };
      return { matched: 2n };
    },
    clearMatch: (request) => {
      asked.cleared = request;
      return { unmatched: 2n };
    },
  };
}

/** Alien's page in Films, for a User with `permissions`, where Heat is a Movie too. */
function openAlien(permissions: Permission[], asked: Asked = {}) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer({ signedIn: permissions, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: ({ movieId }) => ({
        movie: {
          id: movieId,
          title: nameOf(movieId),
          releaseDate: "1979-05-25",
          versions: [{ id: 1n, fileName: "Alien.mkv" }],
        },
      }),
      listMovies: () => ({}),
      listUnmatched: () => ({}),
      listGenres: () => ({}),
    });
    router.service(MatchService, matchService(asked));
  });
  openApp("/libraries/1/movies/tmdb-348", transport);
}

/** The names of the items of the menu of the "…" named `name`, which stays open. */
async function menuItems(name: string) {
  await openMenu(name);
  const found = await screen.findAllByRole("menuitem");
  return found.map((item) => item.textContent);
}

test("only a User with Manage Libraries sees Fix Match and Unmatch", async () => {
  openAlien(newUser);
  const without = await menuItems("More actions");
  expect(without).not.toContain("Fix Match");
  expect(without).not.toContain("Unmatch");
  cleanup();

  openAlien(managesLibraries);

  const items = await menuItems("More actions");
  expect(items).toContain("Fix Match");
  expect(items).toContain("Unmatch");
});

test("fix Match searches for the Movie, shows each title's year and Poster, and goes to the one picked", async () => {
  const asked: Asked = {};
  openAlien(managesLibraries, asked);
  await pickFromMenu("More actions", "Fix Match");
  const dialog = await screen.findByRole("dialog", { name: "Match Alien (1979)" });

  const heat = await within(dialog).findByRole("button", { name: "Match to Heat (1995)" });
  const row = heat.closest("li");
  fireEvent.click(heat);

  expect(asked.searched).toMatchObject({
    kind: TitleKind.MOVIE,
    title: "Alien",
    year: 1979,
    libraryId: 1n,
  });
  expect(row?.querySelector("img")?.getAttribute("src")).toBe(POSTER);
  await expect(screen.findByRole("heading", { name: "Heat" })).resolves.toBeDefined();
  expect(asked.matched).toMatchObject({
    matchedTitle: { libraryId: 1n, id: tmdb("348") },
    titleId: tmdb("949"),
  });
});

test("unmatch, once confirmed, Unmatches the Movie's every Item and goes to its Library", async () => {
  const asked: Asked = {};
  openAlien(managesLibraries, asked);
  await pickFromMenu("More actions", "Unmatch");
  const dialog = await screen.findByRole("alertdialog", { name: "Unmatch Alien (1979)?" });

  fireEvent.click(within(dialog).getByRole("button", { name: "Unmatch" }));

  await expect(screen.findByRole("heading", { name: "Films" })).resolves.toBeDefined();
  expect(asked.cleared?.matchedTitle).toMatchObject({ libraryId: 1n, id: tmdb("348") });
});

test("fix Match on a Show Matches its every Episode and goes to the Show picked", async () => {
  const asked: Asked = {};
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  const transport = fakeServer({ signedIn: managesLibraries, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: ({ showId }) => ({
        show: { id: showId, title: nameOf(showId) },
        seasons: [{ number: 1 }],
      }),
      listEpisodes: () => ({}),
      listUnmatched: () => ({}),
      listShows: () => ({}),
      listGenres: () => ({}),
    });
    router.service(MatchService, matchService(asked));
  });
  openApp("/libraries/2/shows/tmdb-70523", transport);
  await pickFromMenu("More actions", "Fix Match");
  const dialog = await screen.findByRole("dialog", { name: "Match Dark" });

  fireEvent.click(await within(dialog).findByRole("button", { name: "Match to Heat (1995)" }));

  await waitFor(() => {
    expect(asked.searched).toMatchObject({ kind: TitleKind.SHOW, title: "Dark", libraryId: 2n });
  });
  await expect(screen.findByRole("heading", { name: "Heat" })).resolves.toBeDefined();
  expect(asked.matched).toMatchObject({ matchedTitle: { libraryId: 2n, id: tmdb("70523") } });
});
