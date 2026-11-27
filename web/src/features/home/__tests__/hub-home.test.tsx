import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { pluginRow, recentlyAddedRow } from "@/test-home-rows";
import { alien, fakeLinkedServer, heat, linksIn, openHub, startedMovie } from "@/test-hub-home";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A row title for the Movie `id`, `title`, in Library 1. */
function movieTitle(id: typeof alien, title: string) {
  return { movieOrShow: { case: "movie", value: { libraryId: 1n, id, title } } } as const;
}

test("continue Watching at the Hub lists each title once, from the Server it was last watched on", async () => {
  openHub({
    "https://basement.example": fakeLinkedServer({
      listContinueWatching: () => ({ entries: [startedMovie(alien, "Alien", 100n)] }),
    }),
    "https://cabin.example": fakeLinkedServer({
      listContinueWatching: () => ({
        entries: [startedMovie(alien, "Alien", 300n), startedMovie(heat, "Heat", 200n)],
      }),
    }),
  });

  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Alien, 1h 39m left", server: "cabin" },
      { name: "Resume Heat, 1h 39m left", server: "cabin" },
    ]);
  });
});

test("home at the Hub merges each kind of row across the Account's Servers", async () => {
  const trending = pluginRow("discover", "trending", "Trending this week");
  openHub({
    "https://basement.example": fakeLinkedServer({
      listHomeRows: () => ({
        rows: [trending, recentlyAddedRow(1n, "Films", LibraryKind.MOVIES)],
      }),
      listHomeRowTitles: () => ({ titles: [movieTitle(alien, "Alien")] }),
      listMovies: () => ({ movies: [{ id: heat, title: "Heat" }] }),
    }),
    "https://cabin.example": fakeLinkedServer({
      listHomeRows: () => ({
        rows: [recentlyAddedRow(1n, "Movies", LibraryKind.MOVIES), trending],
      }),
      listHomeRowTitles: () => ({
        titles: [movieTitle(heat, "Heat"), movieTitle(alien, "Alien")],
      }),
      listMovies: () => ({ movies: [{ id: alien, title: "Alien" }] }),
    }),
  });

  // Each Server's titles show as it answers, so this waits for both.
  await waitFor(() => {
    expect(linksIn("Trending this week")).toStrictEqual([
      { name: "Alien", server: "basement" },
      { name: "Heat", server: "cabin" },
    ]);
  });
  await waitFor(() => {
    expect(linksIn("Recently added Movies")).toStrictEqual([
      { name: "Heat", server: "basement" },
      { name: "Alien", server: "cabin" },
    ]);
  });
});

test("an unreachable Server's rows are left out, and the others' still show", async () => {
  openHub({
    "https://cabin.example": fakeLinkedServer({
      listContinueWatching: () => ({ entries: [startedMovie(heat, "Heat", 200n)] }),
    }),
  });

  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Heat, 1h 39m left", server: "cabin" },
    ]);
  });
});

/** A fake Server listing Alien on Continue Watching till it's hidden, as `hidden` records. */
function serverHidingAlien(updatedAt: bigint, hidden: Set<string>, origin: string) {
  return fakeLinkedServer(
    {
      listContinueWatching: () => ({
        entries: hidden.has(origin) ? [] : [startedMovie(alien, "Alien", updatedAt)],
      }),
    },
    {
      setHiddenFromContinueWatching: () => {
        hidden.add(origin);
        return {};
      },
    },
  );
}

test("removing a title at the Hub hides it on every Server that lists it", async () => {
  const hidden = new Set<string>();
  const basementAt = "https://basement.example";
  const cabinAt = "https://cabin.example";
  openHub({
    [basementAt]: serverHidingAlien(100n, hidden, basementAt),
    [cabinAt]: serverHidingAlien(300n, hidden, cabinAt),
  });
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toHaveLength(1);
  });

  fireEvent.click(await screen.findByRole("button", { name: "More for Alien" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from recently watched" }));

  await waitFor(() => {
    expect(hidden).toStrictEqual(new Set([basementAt, cabinAt]));
  });
  await waitFor(() => {
    expect(screen.queryByRole("link", { name: /Alien/u })).toBeNull();
  });
});
