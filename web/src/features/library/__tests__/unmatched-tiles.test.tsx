import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { UnmatchedItemSummarySchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { TitleKind } from "@/gen/nama/v1/match_pb";
import { openApp } from "@/test-app";
import { type ServerModel, admin, fakeServer, newUser } from "@/test-server";

afterEach(cleanup);

const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];

const alien = { itemId: 3n, versionId: 7n, kind: TitleKind.MOVIE, title: "Alien", year: 1979 };
const severance = {
  itemId: 4n,
  versionId: 8n,
  kind: TitleKind.SHOW,
  title: "Severance",
  season: 1,
  episode: 2,
};

/**
 * A fake Server, signed in with `signedIn`, whose Library 1 holds the Movie Heat and lists `items`
 * as Unmatched, until `SetMatch` Matches one, which it then stops listing.
 */
function libraryWith(
  items: MessageInitShape<typeof UnmatchedItemSummarySchema>[],
  signedIn: ServerModel["signedIn"] = newUser,
) {
  const matched = new Set<bigint>();
  return fakeServer(
    { signedIn, libraries },
    {
      BrowseService: {
        listGenres: () => ({ genres: ["Crime"] }),
        listMovies: () => ({
          movies: [{ id: { namespace: "tmdb", value: "949" }, title: "Heat" }],
          page: { totalSize: 1 },
        }),
        listUnmatched: () => {
          const left = items.filter(({ itemId }) => !matched.has(itemId ?? 0n));
          return { items: left, page: { totalSize: left.length } };
        },
      },
      MatchService: {
        searchTitles: () => ({
          candidates: [{ id: { namespace: "tmdb", value: "348" }, title: "Alien", year: 1979 }],
        }),
        setMatch: ({ itemId }) => {
          matched.add(itemId);
          return { matched: 1n };
        },
      },
    },
  ).transport;
}

/** Opens the Filter panel, the button that says what's on. */
function openFilter() {
  fireEvent.click(screen.getByRole("button", { name: /^Filter/u }));
}

describe("a Library's Unmatched Items as tiles", () => {
  test("follow its titles, titled by what their names say", async () => {
    openApp("/libraries/1", libraryWith([alien, severance]));

    await screen.findByRole("link", { name: "Play Alien (1979)" });

    expect(
      screen
        .getAllByRole("link")
        .map((link) => link.getAttribute("aria-label"))
        .filter((label) => ["Heat", "Play Alien (1979)"].includes(String(label))),
    ).toStrictEqual(["Heat", "Play Alien (1979)"]);
    expect(screen.getByText("Alien (1979)")).toBeDefined();
    expect(screen.getByText("Severance · Season 1, Episode 2")).toBeDefined();
  });

  test("play for a member, who isn't offered Find a Match", async () => {
    openApp("/libraries/1", libraryWith([alien]));

    await screen.findByRole("link", { name: "Play Alien (1979)" });

    expect(screen.queryByRole("button", { name: "Find a Match" })).toBeNull();
  });

  test("open Find a Match for an Admin", async () => {
    openApp("/libraries/1", libraryWith([alien], admin));

    fireEvent.click(await screen.findByRole("button", { name: "Find a Match" }));

    await expect(screen.findByRole("radio", { name: "Alien (1979)" })).resolves.toBeDefined();
  });

  test("lose a tile once its item is Matched", async () => {
    openApp("/libraries/1", libraryWith([alien], admin));

    fireEvent.click(await screen.findByRole("button", { name: "Find a Match" }));
    await screen.findByRole("radio", { name: "Alien (1979)" });
    fireEvent.click(screen.getByRole("button", { name: "Match" }));

    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "Play Alien (1979)" })).toBeNull();
    });
  });

  test("leave no chip and no tiles in a Library without any", async () => {
    openApp("/libraries/1", libraryWith([]));
    await screen.findByRole("link", { name: "Heat" });

    openFilter();

    await screen.findByRole("button", { name: "Crime" });
    expect(screen.queryByRole("button", { name: /^Not matched/u })).toBeNull();
    expect(screen.queryByRole("link", { name: /^Play /u })).toBeNull();
  });
});

describe("the Not matched chip", () => {
  test("shows the count, and turns the grid to only the tiles, in the URL", async () => {
    const router = openApp("/libraries/1", libraryWith([alien, severance]));
    await screen.findByRole("link", { name: "Heat" });

    openFilter();
    fireEvent.click(await screen.findByRole("button", { name: "Not matched 2" }));

    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "Heat" })).toBeNull();
    });
    expect(screen.getByRole("link", { name: "Play Alien (1979)" })).toBeDefined();
    expect(screen.getByText("2 Not matched")).toBeDefined();
    expect(router.state.location.search).toMatchObject({ unmatched: true });
  });

  test("goes with Clear filters", async () => {
    const router = openApp("/libraries/1?unmatched=1", libraryWith([alien]));
    await screen.findByRole("link", { name: "Play Alien (1979)" });

    openFilter();
    fireEvent.click(await screen.findByRole("button", { name: "Clear filters" }));

    await screen.findByRole("link", { name: "Heat" });
    expect(router.state.location.search).not.toHaveProperty("unmatched");
  });

  test("turns off when a genre is picked, which hides the tiles", async () => {
    openApp("/libraries/1?unmatched=1", libraryWith([alien]));
    await screen.findByRole("link", { name: "Play Alien (1979)" });

    openFilter();
    fireEvent.click(await screen.findByRole("button", { name: "Crime" }));

    await screen.findByRole("link", { name: "Heat" });
    expect(screen.queryByRole("link", { name: "Play Alien (1979)" })).toBeNull();
    // The panel stays open under the grid.
    expect(screen.getByRole("button", { name: "Not matched 1" }).getAttribute("aria-pressed")).toBe(
      "false",
    );
  });
});
