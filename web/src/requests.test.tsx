import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  FoundMovieSchema,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { eventFeed, fakeServer, fill, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

const heat = {
  id: 2n,
  movie: { tmdbId: 949n, title: "Heat", releaseDate: "1995-12-15" },
  state: RequestState.PENDING,
  requesters: [
    { userId: 1n, name: "Sam" },
    { userId: 2n, name: "Ada" },
  ],
};

const dune = {
  id: 1n,
  movie: { tmdbId: 438_631n, title: "Dune" },
  state: RequestState.APPROVED,
  requesters: [{ userId: 3n, name: "Grace" }],
};

function server(
  requests: () => MessageInitShape<typeof RequestSchema>[],
  events = new ReadableStream<SubscribeResponse>(),
) {
  return fakeServer({ signedIn: newUser, events }, {}, {}, (router) => {
    router.service(RequestService, { listRequests: () => ({ requests: requests() }) });
  });
}

test("the Requests page shows each Request's Movie, its state in words and its Requesters", async () => {
  openApp(
    "/requests",
    server(() => [heat, dune]),
  );

  const rows = await screen.findAllByRole("listitem");
  expect(rows.map((row) => row.textContent)).toEqual([
    "Heat1995 · Requested by you and AdaPendingWithdraw",
    "DuneRequested by GraceApproved",
  ]);
});

test("the Requests page says so when there are none", async () => {
  openApp(
    "/requests",
    server(() => []),
  );

  expect(await screen.findByText("No Requests yet.")).toBeDefined();
});

test("a Request made elsewhere shows up without a reload", async () => {
  const events = eventFeed();
  let requests: MessageInitShape<typeof RequestSchema>[] = [];
  openApp(
    "/requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("No Requests yet.");

  requests = [dune];
  events.send({ event: { case: "requestCreated", value: { request: dune } } });

  expect(await screen.findByText("Dune")).toBeDefined();
});

test.each([
  { how: "approved elsewhere", state: RequestState.APPROVED, label: "Approved" },
  { how: "Matched by a scan", state: RequestState.AVAILABLE, label: "Available" },
])("a Request $how shows as $label without a reload", async ({ state, label }) => {
  const events = eventFeed();
  let requests: MessageInitShape<typeof RequestSchema>[] = [heat];
  openApp(
    "/requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("Pending");

  const changed = { ...heat, state };
  requests = [changed];
  events.send({ event: { case: "requestStateChanged", value: { request: changed } } });

  expect(await screen.findByText(label)).toBeDefined();
});

test("a Request cancelled elsewhere goes without a reload", async () => {
  const events = eventFeed();
  let requests = [dune];
  openApp(
    "/requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("Dune");

  requests = [];
  events.send({ event: { case: "requestCancelled", value: { request: dune } } });

  expect(await screen.findByText("No Requests yet.")).toBeDefined();
});

test("a Request withdrawn from elsewhere goes without a reload", async () => {
  const events = eventFeed();
  let requests = [heat];
  openApp(
    "/requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("Heat");

  requests = [];
  events.send({ event: { case: "requestWithdrawn", value: { request: heat } } });

  expect(await screen.findByText("No Requests yet.")).toBeDefined();
});

test("a Declined Request shows its Requesters why", async () => {
  openApp(
    "/requests",
    server(() => [{ ...heat, state: RequestState.DECLINED, declineReason: "Not out on disc yet" }]),
  );

  expect(await screen.findByText("Declined")).toBeDefined();
  expect(screen.getByText("Reason: Not out on disc yet")).toBeDefined();
});

type Found = MessageInitShape<typeof FoundMovieSchema>;

/**
 * A fake Server whose metadata source has `movies`, where asking for one opens a Pending Request
 * for the signed-in User (ID 1), or adds them as a Requester to the one it has.
 */
function searchableServer(movies: Found[], permissions = newUser) {
  const found = new Map(
    movies.map((init) => {
      const movie = create(FoundMovieSchema, init);
      return [movie.tmdbId, movie];
    }),
  );
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      searchMovies: () => ({ movies: [...found.values()] }),
      lookUpMovie: ({ movieTmdbId }) => ({ movie: found.get(movieTmdbId) }),
      createRequest: ({ movieTmdbId }) => {
        const movie = found.get(movieTmdbId) ?? create(FoundMovieSchema);
        movie.request = create(RequestSchema, {
          state: movie.request?.state ?? RequestState.PENDING,
          requesters: [...(movie.request?.requesters ?? []), { userId: 1n, name: "Sam" }],
        });
        return { request: movie.request };
      },
    });
  });
}

const notOnTheServer: Found = { tmdbId: 438_631n, title: "Dune", year: 2021 };
const inALibrary: Found = { tmdbId: 348n, title: "Alien", year: 1979, libraryId: 4n };
const askedForByAda: Found = {
  tmdbId: 949n,
  title: "Heat",
  year: 1995,
  request: { id: 2n, state: RequestState.APPROVED, requesters: [{ userId: 2n, name: "Ada" }] },
};

function search(title: string) {
  fill("Find a Movie", title);
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  return screen.findByRole("region", { name: `Results for “${title}”` });
}

test("a Movie found that isn't on the Server shows as Pending once Requested, without a reload", async () => {
  openApp("/requests", searchableServer([notOnTheServer]));
  await screen.findByLabelText("Find a Movie");

  const results = await search("Dune");
  fireEvent.click(await within(results).findByRole("button", { name: "Request Dune" }));

  expect(await within(results).findByText("Pending")).toBeDefined();
  expect(within(results).queryByRole("button", { name: "Request Dune" })).toBeNull();
});

test("a Movie found in a Library shows as in it, with no Request button", async () => {
  openApp("/requests", searchableServer([inALibrary]));
  await screen.findByLabelText("Find a Movie");

  const results = await search("Alien");

  expect(await within(results).findByText("In a Library")).toBeDefined();
  expect(within(results).queryByRole("button", { name: "Request Alien" })).toBeNull();
  const link = within(results).getByRole("link", { name: /Alien/u });
  expect(link.getAttribute("href")).toBe("/libraries/4/movies/tmdb-348");
});

test("a Movie someone else Requested shows its state, and Requesting it joins", async () => {
  openApp("/requests", searchableServer([askedForByAda]));
  await screen.findByLabelText("Find a Movie");

  const results = await search("Heat");
  expect(await within(results).findByText("Approved")).toBeDefined();
  fireEvent.click(within(results).getByRole("button", { name: "Request Heat" }));

  await waitFor(() => {
    expect(within(results).queryByRole("button", { name: "Request Heat" })).toBeNull();
  });
  expect(within(results).getByText("Approved")).toBeDefined();
});

test("a User who may not Request Movies isn't offered the search", async () => {
  openApp("/requests?title=Dune", searchableServer([notOnTheServer], [Permission.VIEW_REQUESTS]));

  await screen.findByText("No Requests yet.");
  expect(screen.queryByLabelText("Find a Movie")).toBeNull();
  expect(screen.queryByText("Dune")).toBeNull();
});

test("a Movie's page shows its Request state after Requesting it, without a reload", async () => {
  openApp(
    "/movies/tmdb-438631",
    searchableServer([{ ...notOnTheServer, overview: "Paul Atreides travels to Arrakis." }]),
  );

  expect(await screen.findByRole("heading", { name: "Dune" })).toBeDefined();
  expect(screen.getByText("Paul Atreides travels to Arrakis.")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Request Dune" }));

  expect(await screen.findByText("Pending")).toBeDefined();
});

test("a Movie's page for one in a Library opens it there", async () => {
  const router = openApp("/movies/tmdb-348", searchableServer([inALibrary]));

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/4/movies/tmdb-348");
  });
});

test("a Movie's page is not found for a User who may not Request Movies", async () => {
  openApp("/movies/tmdb-438631", searchableServer([notOnTheServer], [Permission.VIEW_REQUESTS]));

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
});

test("Requesting a Movie a scan Matched meanwhile shows it in its Library", async () => {
  const movie = create(FoundMovieSchema, notOnTheServer);
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      searchMovies: () => ({ movies: [movie] }),
      createRequest: () => {
        movie.libraryId = 4n;
        throw refused(Code.FailedPrecondition, "a Library has it", {
          reason: RefusalReason.ALREADY_IN_LIBRARY,
        });
      },
    });
  });
  openApp("/requests", transport);
  await screen.findByLabelText("Find a Movie");

  const results = await search("Dune");
  fireEvent.click(await within(results).findByRole("button", { name: "Request Dune" }));

  expect(await within(results).findByText("In a Library")).toBeDefined();
  expect(within(results).queryByRole("button", { name: "Request Dune" })).toBeNull();
});
