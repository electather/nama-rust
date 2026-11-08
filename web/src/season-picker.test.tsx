import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  FollowedShowSchema,
  FoundSeasonSchema,
  FoundShowSchema,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { Code } from "@connectrpc/connect";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { eventFeed, fakeServer, fill, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

/**
 * A fake Server whose metadata source has `show`, where asking for Seasons opens a Pending Request
 * for each one for the signed-in User (ID 1), or adds them as a Requester to the one it has.
 */
function showServer(
  init: MessageInitShape<typeof FoundShowSchema>,
  permissions: Permission[] = newUser,
  events?: ReadableStream<SubscribeResponse>,
) {
  const show = create(FoundShowSchema, init);
  const asked: number[][] = [];
  const followed: bigint[] = [];
  const state =
    events === undefined ? { signedIn: permissions } : { signedIn: permissions, events };
  const transport = fakeServer(state, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      searchMovies: () => ({}),
      searchShows: () => ({ shows: [show] }),
      lookUpShow: () => ({ show }),
      createSeasonRequests: ({ seasonNumbers }) => {
        if (seasonNumbers.length === 0) {
          throw refused(Code.InvalidArgument, "name at least one Season", {
            reason: RefusalReason.FIELD_REQUIRED,
            field: "season_numbers",
          });
        }
        asked.push(seasonNumbers);
        const seasons = show.seasons.filter(({ seasonNumber }) =>
          seasonNumbers.includes(seasonNumber),
        );
        for (const season of seasons) {
          season.request = create(RequestSchema, {
            state: season.request?.state ?? RequestState.PENDING,
            requesters: [...(season.request?.requesters ?? []), { userId: 1n, name: "Sam" }],
          });
        }
        return { requests: seasons.map(({ request }) => request).filter((r) => r !== undefined) };
      },
      createFollowedShow: ({ showTmdbId }) => {
        followed.push(showTmdbId);
        show.followedShow = create(FollowedShowSchema, {
          state: RequestState.PENDING,
          requesters: [{ userId: 1n, name: "Sam" }],
        });
        return { followedShow: show.followedShow };
      },
    });
  });
  return { transport, asked, followed, show };
}

const dark = {
  tmdbId: 70_523n,
  title: "Dark",
  year: 2017,
  seasons: [
    { seasonNumber: 1, airDate: "2017-12-01" },
    { seasonNumber: 2, airDate: "2019-06-21" },
    { seasonNumber: 3, airDate: "2020-06-27" },
  ],
};

/** The row of the picker that lists the Season named `name`. */
function seasonRow(name: string) {
  const row = screen.getByRole("checkbox", { name }).closest("li");
  if (row === null) throw new Error(`${name} has no row`);
  return row;
}

function checkbox(name: string) {
  return screen.getByRole<HTMLInputElement>("checkbox", { name });
}

test("picking Seasons 1 and 2 and Requesting them shows both as Pending, without a reload", async () => {
  const { transport, asked } = showServer(dark);
  openApp("/shows/tmdb-70523", transport);
  expect(await screen.findByRole("heading", { name: "Dark" })).toBeDefined();

  fireEvent.click(checkbox("Season 1"));
  fireEvent.click(checkbox("Season 2"));
  fireEvent.click(screen.getByRole("button", { name: "Request Seasons of Dark" }));

  expect(await within(seasonRow("Season 1")).findByText("Pending")).toBeDefined();
  expect(within(seasonRow("Season 2")).getByText("Pending")).toBeDefined();
  expect(within(seasonRow("Season 3")).queryByText("Pending")).toBeNull();
  expect(asked).toEqual([[1, 2]]);
  expect(checkbox("Season 1").disabled).toBe(true);
  expect(checkbox("Season 1").checked).toBe(false);
  expect(checkbox("Season 3").disabled).toBe(false);
});

test("a Season someone else Requests meanwhile shows its state from the Events stream", async () => {
  const events = eventFeed();
  const { transport, show } = showServer(dark, newUser, events.stream);
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  const request = create(RequestSchema, {
    id: 5n,
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
  });
  show.seasons[2] = create(FoundSeasonSchema, { seasonNumber: 3, request });
  events.send({ event: { case: "requestCreated", value: { request } } });

  expect(await within(seasonRow("Season 3")).findByText("Pending")).toBeDefined();
  expect(checkbox("Season 3").disabled).toBe(false);
});

test("a Season in a Library shows as in it and can't be picked", async () => {
  const { transport } = showServer({
    ...dark,
    seasons: [{ seasonNumber: 1, inLibrary: true }, { seasonNumber: 2 }],
  });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  expect(within(seasonRow("Season 1")).getByText("In a Library")).toBeDefined();
  expect(checkbox("Season 1").disabled).toBe(true);
  expect(screen.getByRole("checkbox", { description: "In a Library" })).toBe(checkbox("Season 1"));
  expect(checkbox("Season 2").disabled).toBe(false);
});

test("a Season someone else Requested shows its state, and Requesting it joins", async () => {
  const { transport, asked } = showServer({
    ...dark,
    seasons: [
      {
        seasonNumber: 1,
        request: {
          id: 2n,
          state: RequestState.APPROVED,
          requesters: [{ userId: 2n, name: "Ada" }],
        },
      },
    ],
  });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });
  expect(within(seasonRow("Season 1")).getByText("Approved")).toBeDefined();

  fireEvent.click(checkbox("Season 1"));
  fireEvent.click(screen.getByRole("button", { name: "Request Seasons of Dark" }));

  await waitFor(() => {
    expect(checkbox("Season 1").disabled).toBe(true);
  });
  expect(asked).toEqual([[1]]);
  expect(within(seasonRow("Season 1")).getByText("Approved")).toBeDefined();
});

test("Requesting with no Season picked says to pick one", async () => {
  const { transport, asked } = showServer(dark);
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  fireEvent.click(screen.getByRole("button", { name: "Request Seasons of Dark" }));

  expect((await screen.findByRole("alert")).textContent).toBe("Pick at least one Season.");
  expect(asked).toEqual([]);
});

test("a Show you follow, whose every Season is in a Library or yours, offers no Request", async () => {
  const yours = { state: RequestState.PENDING, requesters: [{ userId: 1n }] };
  const { transport } = showServer({
    ...dark,
    seasons: [
      { seasonNumber: 1, inLibrary: true },
      { seasonNumber: 2, request: yours },
    ],
    followedShow: yours,
  });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  expect(screen.queryByRole("button", { name: "Request Seasons of Dark" })).toBeNull();
});

test("a search finds Shows, which open their page to pick Seasons", async () => {
  const { transport } = showServer(dark);
  openApp("/requests", transport);

  await screen.findByLabelText("Find a Movie or Show");
  fill("Find a Movie or Show", "Dark");
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  const shows = await screen.findByRole("region", { name: "Shows for “Dark”" });

  const link = await within(shows).findByRole("link", { name: /Dark/u });
  expect(link.getAttribute("href")).toBe("/shows/tmdb-70523");
});

test("a User who may Request only Shows searches only Shows", async () => {
  const { transport } = showServer(dark, [Permission.REQUEST_TV]);
  openApp("/requests", transport);

  await screen.findByLabelText("Find a Show");
  fill("Find a Show", "Dark");
  fireEvent.click(screen.getByRole("button", { name: "Search" }));

  expect(await screen.findByRole("region", { name: "Shows for “Dark”" })).toBeDefined();
  expect(screen.queryByRole("region", { name: "Movies for “Dark”" })).toBeNull();
});

test("a Show's page is not found for a User who may not Request Shows", async () => {
  const { transport } = showServer(dark, [Permission.REQUEST_MOVIE]);
  openApp("/shows/tmdb-70523", transport);

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
});

test("picking Whole Show leaves out the Seasons, and Requesting it shows it Pending", async () => {
  const { transport, asked, followed } = showServer(dark);
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });

  fireEvent.click(checkbox("Season 1"));
  fireEvent.click(checkbox("Whole Show"));
  const seasonsWhilePicked = [checkbox("Season 1").disabled, checkbox("Season 3").disabled];
  fireEvent.click(screen.getByRole("button", { name: "Request Seasons of Dark" }));

  expect(await within(seasonRow("Whole Show")).findByText("Pending")).toBeDefined();
  expect(seasonsWhilePicked).toEqual([true, true]);
  expect(followed).toEqual([70_523n]);
  expect(asked).toEqual([]);
  expect(checkbox("Whole Show").disabled).toBe(true);
});

test("a Show someone else follows shows its state, and Requesting the Whole Show joins it", async () => {
  const { transport, followed } = showServer({
    ...dark,
    followedShow: { state: RequestState.APPROVED, requesters: [{ userId: 2n, name: "Ada" }] },
  });
  openApp("/shows/tmdb-70523", transport);
  await screen.findByRole("heading", { name: "Dark" });
  expect(within(seasonRow("Whole Show")).getByText("Approved")).toBeDefined();

  fireEvent.click(checkbox("Whole Show"));
  fireEvent.click(screen.getByRole("button", { name: "Request Seasons of Dark" }));

  await waitFor(() => {
    expect(checkbox("Whole Show").disabled).toBe(true);
  });
  expect(followed).toEqual([70_523n]);
});
