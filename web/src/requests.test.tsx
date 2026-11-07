import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { RequestService, RequestState, type RequestSchema } from "@/gen/nama/v1/request_pb";

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
    "Heat1995 · Requested by you and AdaPending",
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
