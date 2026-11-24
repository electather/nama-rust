import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, type LibraryService } from "@/gen/nama/v1/library_pb";
import { librariesQuery } from "@/shared/hooks/use-libraries";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

/** The Details' Library fact, if the page shows one. */
function libraryFact() {
  return screen.queryByText("Library", { selector: "dt" });
}

const titles = [
  { kind: "Movie", path: "/libraries/1/movies/tmdb-496243", title: "Parasite" },
  { kind: "Show", path: "/libraries/1/shows/tmdb-70523", title: "Dark" },
];

describe.each(titles)("a $kind's page", ({ path, title }) => {
  /**
   * Opens the title's page, in Library 1, on a Server whose Libraries `listLibraries` lists, and
   * returns how the app's query of them stands.
   */
  function open(listLibraries: ServiceImpl<typeof LibraryService>["listLibraries"]) {
    const transport = fakeServer(
      { signedIn: newUser, libraryService: { listLibraries } },
      {},
      {},
      (router) => {
        router.service(BrowseService, {
          getMovie: () => ({ movie: { title, versions: [{ id: 1n }] } }),
          getShow: () => ({ show: { title }, seasons: [] }),
          listEpisodes: () => ({ episodes: [] }),
        });
      },
    );
    const { queryClient } = openApp(path, transport).options.context;
    return () => queryClient.getQueryState(librariesQuery(transport).queryKey)?.status;
  }

  test("shows without waiting on the Libraries, and no Library until they load", async () => {
    // oxlint-disable-next-line require-await -- never answers, so the Libraries stay loading
    open(async () => new Promise<never>(() => {}));

    await expect(screen.findByRole("heading", { level: 1, name: title })).resolves.toBeDefined();
    expect(libraryFact()).toBeNull();
  });

  test("shows, without a Library, when the Libraries fail to load", async () => {
    // `internal`, as the app retries only a Server it can't reach.
    const librariesStatus = open(() => {
      throw new ConnectError("broken", Code.Internal);
    });

    await waitFor(() => {
      expect(librariesStatus()).toBe("error");
    });
    await expect(screen.findByRole("heading", { level: 1, name: title })).resolves.toBeDefined();
    expect(libraryFact()).toBeNull();
  });

  test("links to its Library once the Libraries load", async () => {
    const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
    open(() => ({ libraries }));

    const details = await screen.findByRole("region", { name: "Details" });
    const link = await within(details).findByRole("link", { name: "Films" });
    expect(link.getAttribute("href")).toBe("/libraries/1");
  });
});
