import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  DownloadStatus,
  FoundSeasonSchema,
  FoundShowSchema,
  RequestSchema,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { eventFeed, openApp } from "@/test-app";
import { layOutShelves } from "@/test-layout";
import { refused } from "@/test-refusal";
import { fakeServer, newUser } from "@/test-server";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/**
 * A fake Server whose title source has `show`, where asking for Seasons opens a Pending Request
 * for each one for the signed-in User (ID 1), or adds them as a Requester to the one it has, unless
 * `refusal` refuses it.
 */
function showServer(
  init: MessageInitShape<typeof FoundShowSchema>,
  { permissions = newUser, events, refusal }: ServerOptions = {},
) {
  const show = create(FoundShowSchema, init);
  const asked: number[][] = [];
  const { transport } = fakeServer(
    { signedIn: permissions, ...(events && { events }) },
    {
      RequestService: {
        lookUpShow: () => ({ show }),
        createSeasonRequests: ({ seasonNumbers }) => {
          if (refusal) {
            throw refusal;
          }
          asked.push(seasonNumbers);
          const seasons = show.seasons.filter(({ seasonNumber }) =>
            seasonNumbers.includes(seasonNumber),
          );
          for (const opened of seasons) {
            opened.request = create(RequestSchema, {
              state: opened.request?.state ?? RequestState.PENDING,
              requesters: [...(opened.request?.requesters ?? []), { userId: 1n, name: "Sam" }],
            });
          }
          return {
            requests: seasons.map(({ request }) => request).filter((r) => r !== undefined),
          };
        },
      },
    },
  );
  return { transport, asked, show };
}

interface ServerOptions {
  permissions?: Permission[];
  events?: ReadableStream<SubscribeResponse>;
  refusal?: Error;
}

/** Episode `episodeNumber` as the title source lists it. */
function chapter(episodeNumber: number) {
  return { episodeNumber, title: `Chapter ${episodeNumber}` };
}

/** Season `seasonNumber` with two Episodes, as the title source lists it, and `more`. */
function season(seasonNumber: number, more: MessageInitShape<typeof FoundSeasonSchema> = {}) {
  return { seasonNumber, episodes: [chapter(1), chapter(2)], ...more };
}

const dark = {
  id: { namespace: "tmdb", value: "70523" },
  title: "Dark",
  year: 2017,
  seasons: [season(1), season(2), season(3)],
};

/** The card of Episode 1 of Season `number`. */
function firstOf(number: number) {
  const card = screen.getByText(new RegExp(`^Season ${number}, Episode 1:`, "u")).closest("li");
  if (card === null) {
    throw new Error(`Season ${number} has no card`);
  }
  return within(card);
}

test("every Episode shows dimmed, and Request Season 1 shows its Season Pending, without a reload", async () => {
  const { transport, asked } = showServer(dark);
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });
  expect(screen.getByText("Season 2, Episode 2: Chapter 2, Not on this Server")).toBeDefined();

  fireEvent.click(screen.getByRole("button", { name: "Request Season 1 of Dark" }));

  await expect(
    screen.findByText("Season 1, Episode 1: Chapter 1, Pending approval"),
  ).resolves.toBeDefined();
  expect(asked).toStrictEqual([[1]]);
  expect(firstOf(1).queryByRole("button")).toBeNull();
  expect(firstOf(2).getByRole("button", { name: "Request Season 2" })).toBeDefined();
  expect(screen.queryByRole("button", { name: "Request Season 1 of Dark" })).toBeNull();
});

test("the + on a missing Episode Requests its Season", async () => {
  const { transport, asked } = showServer(dark);
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  fireEvent.click(firstOf(3).getByRole("button", { name: "Request Season 3" }));

  await waitFor(() => {
    expect(asked).toStrictEqual([[3]]);
  });
});

test("the … Requests every Season at once", async () => {
  const { transport, asked } = showServer({
    ...dark,
    seasons: [season(0), season(1), season(2, { inLibrary: true }), season(3)],
  });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  fireEvent.click(screen.getByRole("button", { name: "More actions" }));
  expect(screen.getByRole("menuitem", { name: "Request Unwatched" })).toBeDefined();
  fireEvent.click(screen.getByRole("menuitem", { name: "Request All Seasons" }));

  await waitFor(() => {
    expect(asked).toStrictEqual([[1, 3]]);
  });
});

test("a Season someone else Requests meanwhile shows its state from the Events stream", async () => {
  const events = eventFeed();
  const { transport, show } = showServer(dark, { events: events.stream });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  const request = create(RequestSchema, {
    id: 5n,
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
  });
  show.seasons[2] = create(FoundSeasonSchema, season(3, { request }));
  events.send({ event: { case: "requestCreated", value: { request } } });

  await expect(
    screen.findByText("Season 3, Episode 1: Chapter 1, Pending approval"),
  ).resolves.toBeDefined();
  expect(firstOf(3).getByText("Pending approval", { selector: ".sr-only" })).toBeDefined();
});

test("a Processing Season's Episodes show a ring filled to how far its downloads have got", async () => {
  const request = {
    id: 7n,
    state: RequestState.PROCESSING,
    requesters: [{ userId: 2n, name: "Ada" }],
    downloads: [
      { title: "Dark.S02E01", percent: 100, status: DownloadStatus.IMPORTING },
      { title: "Dark.S02E02", percent: 40, status: DownloadStatus.DOWNLOADING },
    ],
  };
  const { transport } = showServer({ ...dark, seasons: [season(1), season(2, { request })] });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  const ring = firstOf(2).getByRole("progressbar", { name: "Downloading" });
  expect(ring.getAttribute("aria-valuenow")).toBe("70");
  expect(firstOf(1).queryByRole("progressbar")).toBeNull();
});

test("an Episode in a Library isn't dimmed, and the main button Requests the next Season", async () => {
  const inLibrary = season(1, {
    inLibrary: true,
    episodes: [{ episodeNumber: 1, title: "Secrets", inLibrary: true }],
  });
  const { transport } = showServer({ ...dark, seasons: [inLibrary, season(2)] });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  expect(firstOf(1).queryByRole("button")).toBeNull();
  expect(screen.getByText("Season 1, Episode 1: Secrets")).toBeDefined();
  expect(screen.getByRole("button", { name: "Request Season 2 of Dark" })).toBeDefined();
});

test("a Season someone else Requested shows its state, and Requesting it joins", async () => {
  const request = { id: 2n, state: RequestState.APPROVED, requesters: [{ userId: 2n }] };
  const { transport, asked } = showServer({ ...dark, seasons: [season(1, { request })] });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });
  expect(screen.getByText("Season 1, Episode 1: Chapter 1, Requested")).toBeDefined();

  fireEvent.click(screen.getByRole("button", { name: "Request Season 1 of Dark" }));

  await waitFor(() => {
    expect(screen.queryByRole("button", { name: "Request Season 1 of Dark" })).toBeNull();
  });
  expect(asked).toStrictEqual([[1]]);
});

test("a refused Request says why", async () => {
  const refusal = refused(Code.FailedPrecondition, "in a Library", {
    reason: RefusalReason.ALREADY_IN_LIBRARY,
  });
  const { transport } = showServer(dark, { refusal });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  fireEvent.click(firstOf(2).getByRole("button", { name: "Request Season 2" }));

  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toBe("One of those Seasons is already in a Library.");
});

test("a Show whose every Season is in a Library or yours offers no Request", async () => {
  const yours = { state: RequestState.PENDING, requesters: [{ userId: 1n }] };
  const { transport } = showServer({
    ...dark,
    seasons: [season(1, { inLibrary: true }), season(2, { request: yours })],
  });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  expect(screen.queryByRole("button", { name: /^Request/u })).toBeNull();
  expect(screen.queryByRole("button", { name: "More actions" })).toBeNull();
});

test("a Show's page is not found for a User who may not Request Shows", async () => {
  const { transport } = showServer(dark, { permissions: [Permission.REQUEST_MOVIE] });
  openApp("/shows/tmdb-70523", transport);

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});

test("a Show to Request shows its Score, genres and Content Rating, its Watchlist state, its Original audio in Details, and its Trailers", async () => {
  const trailers = [{ name: "Final Trailer", url: "https://www.youtube.com/watch?v=darkFinal01" }];
  const { transport } = showServer({
    ...dark,
    score: 8.4,
    originalLanguage: "de",
    genres: ["Crime", "Drama"],
    contentRating: "TV-MA",
    onWatchlist: true,
    trailers,
  });
  openApp("/shows/tmdb-70523", transport);

  const score = await screen.findByText("Score");
  expect(score.closest("p")?.textContent).toBe("Score8.42017TV-MA");
  expect(screen.getByText("Show · Crime · Drama")).toBeDefined();
  expect(screen.getByRole("button", { name: "Remove from your Watchlist" })).toBeDefined();
  const audio = screen.getByText("Original audio", { selector: "dt" });
  expect(audio.nextElementSibling?.textContent).toBe("German");
  expect(screen.getByRole("link", { name: "Final Trailer" }).getAttribute("href")).toBe(
    trailers[0]?.url,
  );
});

test("a Show to Request shows who stars in it, its Cast & Crew, and who created it", async () => {
  const cast = [{ name: "Louis Hofmann", part: "Jonas Kahnwald" }];
  const crew = [{ name: "Baran bo Odar", part: "Creator" }];
  const { transport } = showServer({ ...dark, cast, crew });
  openApp("/shows/tmdb-70523", transport);

  const starring = await screen.findByText(/^Starring/u);
  expect(starring.textContent).toBe("Starring Louis Hofmann");
  const row = within(screen.getByRole("list", { name: "Cast & Crew" }));
  expect(row.getAllByRole("listitem")).toHaveLength(2);
  const created = screen.getByText("Created by", { selector: "dt" });
  expect(created.nextElementSibling?.textContent).toBe("Baran bo Odar");
});
