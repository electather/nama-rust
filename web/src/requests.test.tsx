import { type MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { RequestSchema, RequestService, RequestState } from "@/gen/nama/v1/request_pb";

import { eventFeed, fakeServer, newUser, openApp } from "./test-app";

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

/** A Season Request of Dark, asked for by Sam, the signed-in User. */
function darkSeason(id: bigint, seasonNumber: number, state: RequestState) {
  return {
    id,
    season: { showTmdbId: 70_523n, seasonNumber, showTitle: "Dark", airDate: "2017-12-01" },
    state,
    requesters: [{ userId: 1n, name: "Sam" }],
  };
}

test("the Requests page groups a Show's Season Requests, in Season order, with their states in words", async () => {
  openApp(
    "/requests",
    server(() => [
      darkSeason(4n, 2, RequestState.APPROVED),
      heat,
      darkSeason(3n, 1, RequestState.PENDING),
      dune,
    ]),
  );

  const seasons = await screen.findByRole("list", { name: "Dark" });
  expect(
    within(seasons)
      .getAllByRole("listitem")
      .map((row) => row.textContent),
  ).toEqual([
    "Season 12017 · Requested by youPendingWithdraw",
    "Season 22017 · Requested by youApproved",
  ]);
  // Each row's title is its first paragraph.
  const titles = screen.getAllByRole("listitem").map((row) => row.querySelector("p")?.textContent);
  expect(titles).toEqual(["Dark", "Season 1", "Season 2", "Heat", "Dune"]);
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
  { how: "taken by its Fulfiller", state: RequestState.PROCESSING, label: "Processing" },
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
