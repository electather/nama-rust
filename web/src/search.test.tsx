import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, fill, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** A fake Server with a Movies Library (1) and a Shows Library (2), browsed through `browse`. */
function serverToSearch(browse: Partial<ServiceImpl<typeof BrowseService>>) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, browse);
  });
}

const theOffice = { tmdb: 2316n } as const;

/** Finds The Office and one of its Episodes for any query with "office" in it, and else nothing. */
const searchLibraries: ServiceImpl<typeof BrowseService>["searchLibraries"] = ({ query }) =>
  query.includes("office")
    ? {
        shows: [
          {
            libraryId: 2n,
            id: { namespace: "tmdb", value: String(theOffice.tmdb) },
            title: "The Office",
          },
        ],
        episodes: [
          {
            libraryId: 2n,
            id: { namespace: "tmdb", value: "9001" },
            showId: { namespace: "tmdb", value: String(theOffice.tmdb) },
            showTitle: "The Office",
            seasonNumber: 2,
            episodeNumber: 1,
            title: "The Dundies: Office Party",
          },
        ],
      }
    : {};

/** Types `query` into the nav's search box, once the app shows it. */
async function search(query: string) {
  await screen.findByLabelText("Search your Libraries");
  fill("Search your Libraries", query);
}

test("typing in the nav's search box lists the Shows and Episodes it finds", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  await search("office");

  const show = await screen.findByRole("link", { name: /^The Office/u });
  expect(show.getAttribute("href")).toBe("/libraries/2/shows/tmdb-2316");
  const episode = screen.getByRole("link", { name: /The Dundies: Office Party/u });
  expect(episode.getAttribute("href")).toBe("/libraries/2/shows/tmdb-2316?season=2");
});

test("picking an Episode found opens its Show at that Season", async () => {
  openApp(
    "/",
    serverToSearch({
      searchLibraries,
      getShow: () => ({ show: { title: "The Office" }, seasons: [{ number: 1 }, { number: 2 }] }),
      listEpisodes: ({ seasonNumber }) => ({
        episodes: [
          {
            seasonNumber,
            episodeNumber: 1,
            title: `Opener of Season ${seasonNumber}`,
            versions: [{ id: 1n }],
          },
        ],
      }),
    }),
  );
  await search("office");

  fireEvent.click(await screen.findByRole("link", { name: /The Dundies: Office Party/u }));

  expect(await screen.findByText(/Opener of Season 2/u)).toBeDefined();
});

test("reloading a search's page shows what it found", async () => {
  openApp("/search?query=office", serverToSearch({ searchLibraries }));

  expect(await screen.findByRole("link", { name: /^The Office/u })).toBeDefined();
  expect(screen.getByLabelText<HTMLInputElement>("Search your Libraries").value).toBe("office");
});

test("a search that finds nothing says so", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  await search("dune");

  expect(await screen.findByText("Nothing found for “dune”.")).toBeDefined();
});

test("a number typed into the URL is searched for as words", async () => {
  const searched: string[] = [];
  openApp(
    "/search?query=1917",
    serverToSearch({
      searchLibraries: ({ query }) => {
        searched.push(query);
        return {};
      },
    }),
  );

  expect(await screen.findByText("Nothing found for “1917”.")).toBeDefined();
  expect(searched).toStrictEqual(["1917"]);
});

test("a blank query in the URL searches for nothing", async () => {
  openApp("/search?query=%20%20", serverToSearch({ searchLibraries }));

  expect(await screen.findByRole("heading", { name: "Search" })).toBeDefined();
});

/** A Movie of the Movies Library a search finds, called `title`, with the TMDB id `tmdb`. */
function movieFound(tmdb: number, title: string) {
  return { libraryId: 1n, id: { namespace: "tmdb", value: String(tmdb) }, title };
}

/** Finds `pages` of Movies in turn, whatever the query, with each page's index as its token. */
function pagingSearch(
  pages: ReturnType<typeof movieFound>[][],
): ServiceImpl<typeof BrowseService>["searchLibraries"] {
  return ({ page }) => {
    const index = Number(page?.token ?? "");
    const next = index + 1 < pages.length ? String(index + 1) : "";
    return { movies: pages[index] ?? [], page: { nextToken: next } };
  };
}

test("Show more lists the next page of what a search found below the first", async () => {
  const pages = [[movieFound(2000, "The Office Party")], [movieFound(1542, "Office Space")]];
  openApp("/search?query=office", serverToSearch({ searchLibraries: pagingSearch(pages) }));

  fireEvent.click(await screen.findByRole("button", { name: "Show more" }));

  await screen.findByRole("link", { name: /^Office Space/u });
  const movies = within(screen.getByRole("region", { name: "Movies" })).getAllByRole("link");
  expect(movies.map((link) => link.getAttribute("href"))).toStrictEqual([
    "/libraries/1/movies/tmdb-2000",
    "/libraries/1/movies/tmdb-1542",
  ]);
  expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
});
