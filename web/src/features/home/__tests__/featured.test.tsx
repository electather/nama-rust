import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type FeaturedTitleSchema } from "@/gen/nama/v1/browse_pb";
import { TitleKind } from "@/gen/nama/v1/match_pb";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { heldBack } from "@/test-held-back";

afterEach(cleanup);

const arrival = {
  id: { namespace: "tmdb", value: "329865" },
  title: "Arrival",
  releaseDate: "2016-11-11",
  runtimeMinutes: 116,
  genres: ["Science Fiction", "Drama", "Mystery"],
  contentRating: "PG-13",
  overview: "A linguist is asked to talk to the beings inside twelve spacecraft.",
  watchState: { positionMs: 4_680_000n, durationMs: 6_960_000n },
};
const severance = {
  id: { namespace: "tmdb", value: "95396" },
  title: "Severance",
  firstAirDate: "2022-02-18",
  seasonCount: 2,
  genres: ["Drama"],
};

/** Home with `titles` Featured, each Watchlist change asked of the Server noted in `asked`. */
function openHome(
  titles: MessageInitShape<typeof FeaturedTitleSchema>[],
  asked: { added: boolean; kind: TitleKind; id: string | undefined }[] = [],
) {
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(BrowseService, {
      listFeatured: () => ({ titles }),
      listContinueWatching: () => ({}),
      listHomeRows: () => ({}),
    });
    router.service(WatchlistService, {
      addToWatchlist: ({ kind, id }) => {
        asked.push({ added: true, kind, id: id?.value });
        return {};
      },
      removeFromWatchlist: ({ kind, id }) => {
        asked.push({ added: false, kind, id: id?.value });
        return {};
      },
    });
  });
  openApp("/", transport);
}

/** The panel of Featured whose button shows `title`. */
function panelOf(title: string) {
  const panel = screen.getByRole("button", { name: `Show ${title}` }).closest("li");
  if (!panel) {
    throw new Error(`no panel for ${title}`);
  }
  return panel;
}

test("the first Featured title opens first, resuming what you started", async () => {
  openHome([
    { libraryId: 1n, movieOrShow: { case: "movie", value: arrival } },
    { libraryId: 2n, movieOrShow: { case: "show", value: severance } },
  ]);

  await screen.findByRole("heading", { level: 1, name: "Arrival" });
  const panel = panelOf("Arrival");
  expect(screen.getByRole("button", { name: "Show Arrival" }).getAttribute("aria-current")).toBe(
    "true",
  );
  expect(within(panel).getByText("PG-13")).toBeDefined();
  expect(within(panel).getByText("2016 · 1h 56m · Science Fiction, Drama")).toBeDefined();
  expect(within(panel).getByRole("link", { name: "Resume, 38m left" }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-329865/play",
  );
});

test("hovering a closed panel opens it, and a Show plays its next Episode", async () => {
  const next = { seasonNumber: 1, episodeNumber: 3, title: "In Perpetuity" };
  openHome([
    { libraryId: 1n, movieOrShow: { case: "movie", value: arrival } },
    { libraryId: 2n, movieOrShow: { case: "show", value: severance }, nextEpisode: next },
  ]);

  fireEvent.mouseEnter(await screen.findByRole("button", { name: "Show Severance" }));

  const panel = panelOf("Severance");
  expect(panel.dataset["open"]).toBe("");
  expect(panelOf("Arrival").dataset["open"]).toBeUndefined();
  expect(within(panel).getByText("2022 · 2 Seasons · Drama")).toBeDefined();
  expect(within(panel).getByRole("link", { name: "Play" }).getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-95396/seasons/1/episodes/3/play",
  );
});

test("a Show with nothing left to play opens its page from More info", async () => {
  openHome([{ libraryId: 2n, movieOrShow: { case: "show", value: severance } }]);

  const moreInfo = await screen.findByRole("link", { name: "More info" });
  expect(moreInfo.getAttribute("href")).toBe("/libraries/2/shows/tmdb-95396");
});

test("the Watchlist button adds a title, or takes it off once it's on", async () => {
  const asked: { added: boolean; kind: TitleKind; id: string | undefined }[] = [];
  openHome(
    [
      { libraryId: 1n, movieOrShow: { case: "movie", value: arrival } },
      { libraryId: 2n, movieOrShow: { case: "show", value: { ...severance, onWatchlist: true } } },
    ],
    asked,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Add to your Watchlist" }));
  fireEvent.focus(screen.getByRole("button", { name: "Show Severance" }));
  const remove = within(panelOf("Severance")).getByRole("button", {
    name: "Remove from your Watchlist",
  });
  fireEvent.click(remove);

  await waitFor(() => {
    expect(asked).toStrictEqual([
      { added: true, kind: TitleKind.MOVIE, id: "329865" },
      { added: false, kind: TitleKind.SHOW, id: "95396" },
    ]);
  });
});

test("featured shows nothing when there's nothing to feature", async () => {
  openHome([]);

  await waitFor(() => {
    expect(screen.queryByRole("region", { name: "Featured" })).toBeNull();
  });
  expect(screen.queryByRole("heading", { level: 1 })).toBeNull();
});

test("the Watchlist button turns to Remove as you press it, and back if the Server refuses", async () => {
  const adding = heldBack();
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(BrowseService, {
      listFeatured: () => ({
        titles: [{ libraryId: 1n, movieOrShow: { case: "movie", value: arrival } }],
      }),
      listContinueWatching: () => ({}),
      listHomeRows: () => ({}),
    });
    router.service(WatchlistService, { addToWatchlist: adding.answer });
  });
  openApp("/", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Add to your Watchlist" }));
  await expect(
    screen.findByRole("button", { name: "Remove from your Watchlist" }),
  ).resolves.toBeDefined();
  adding.refuse();

  await expect(
    screen.findByRole("button", { name: "Add to your Watchlist" }),
  ).resolves.toBeDefined();
});

test("clicking the open panel opens its title's page, and a tap on a closed one only opens it", async () => {
  openHome([
    { libraryId: 1n, movieOrShow: { case: "movie", value: arrival } },
    { libraryId: 2n, movieOrShow: { case: "show", value: severance } },
  ]);

  const severancePanel = await screen.findByRole("button", { name: "Show Severance" });
  fireEvent.pointerDown(severancePanel);
  fireEvent.mouseEnter(severancePanel);
  fireEvent.click(severancePanel, { detail: 1 });
  expect(panelOf("Severance").dataset["open"]).toBe("");
  expect(location.pathname).toBe("/");

  fireEvent.pointerDown(severancePanel);
  fireEvent.click(severancePanel, { detail: 1 });

  await waitFor(() => {
    expect(location.pathname).toBe("/libraries/2/shows/tmdb-95396");
  });
});
