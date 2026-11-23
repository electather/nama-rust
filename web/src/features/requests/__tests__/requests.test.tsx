import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import {
  type FollowedShowSchema,
  type RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { eventFeed, fakeServer, newUser, openApp, requestTitles } from "@/test-app";

afterEach(cleanup);

const heat = {
  id: 2n,
  movieOrSeason: {
    case: "movie" as const,
    value: { id: { namespace: "tmdb", value: "949" }, title: "Heat", releaseDate: "1995-12-15" },
  },
  state: RequestState.PENDING,
  requesters: [
    { userId: 1n, name: "Sam" },
    { userId: 2n, name: "Ada" },
  ],
};

const dune = {
  id: 1n,
  movieOrSeason: {
    case: "movie" as const,
    value: { id: { namespace: "tmdb", value: "438631" }, title: "Dune" },
  },
  state: RequestState.APPROVED,
  requesters: [{ userId: 3n, name: "Grace" }],
  quality: { id: 7n, name: "4K", profileId: "5" },
};

function server(
  requests: () => MessageInitShape<typeof RequestSchema>[],
  events = new ReadableStream<SubscribeResponse>(),
  followedShows: () => MessageInitShape<typeof FollowedShowSchema>[] = () => [],
) {
  return fakeServer({ signedIn: newUser, events }, {}, {}, (router) => {
    router.service(RequestService, {
      listDefaultQualities: () => ({}),
      listRequests: () => ({ requests: requests(), followedShows: followedShows() }),
    });
  });
}

test("the Requests page shows each Request's Movie, its Quality, its state in words and its Requesters", async () => {
  openApp(
    "/?settings=requests",
    server(() => [heat, dune]),
  );

  const rows = await within(
    await screen.findByRole("list", { name: /^(?:Your|All) Requests$/u }),
  ).findAllByRole("listitem");
  expect(rows.map((row) => row.textContent)).toStrictEqual([
    "HeatPending1995 · Requested by you and Ada",
    "DuneApproved4K · Requested by Grace",
  ]);
});

/** A Season Request of Dark, asked for by Sam, the signed-in User. */
function darkSeason(id: bigint, seasonNumber: number, state: RequestState) {
  return {
    id,
    movieOrSeason: {
      case: "season" as const,
      value: {
        showId: { namespace: "tmdb", value: "70523" },
        seasonNumber,
        showTitle: "Dark",
        airDate: "2017-12-01",
      },
    },
    state,
    requesters: [{ userId: 1n, name: "Sam" }],
  };
}

test("the Requests page groups a Show's Season Requests, in Season order, with their states in words", async () => {
  openApp(
    "/?settings=requests",
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
  ).toStrictEqual([
    "Season 1Pending2017 · Requested by you",
    "Season 2Approved2017 · Requested by you",
  ]);
  expect(requestTitles()).toStrictEqual(["Dark", "Season 1", "Season 2", "Heat", "Dune"]);
});

/** Sam's Followed Show of Dark, Approved. */
const followedDark = {
  id: 7n,
  showId: { namespace: "tmdb", value: "70523" },
  showTitle: "Dark",
  state: RequestState.APPROVED,
  requesters: [{ userId: 1n, name: "Sam" }],
};

test("a Followed Show lists above its Show's Season Requests", async () => {
  openApp(
    "/?settings=requests",
    server(
      () => [heat, { ...darkSeason(3n, 1, RequestState.APPROVED), followedShowId: 7n }],
      undefined,
      () => [followedDark],
    ),
  );

  const dark = await screen.findByRole("list", { name: "Dark" });
  expect(
    within(dark)
      .getAllByRole("listitem")
      .map((row) => row.textContent),
  ).toStrictEqual([
    "Followed ShowApprovedRequested by you",
    "Season 1Approved2017 · Requested by you",
  ]);
});

test("a Season Request a scan opens for a Followed Show shows up under it without a reload", async () => {
  const events = eventFeed();
  let requests: MessageInitShape<typeof RequestSchema>[] = [];
  openApp(
    "/?settings=requests",
    server(
      () => requests,
      events.stream,
      () => [followedDark],
    ),
  );
  const dark = await screen.findByRole("list", { name: "Dark" });

  const season3 = { ...darkSeason(9n, 3, RequestState.AVAILABLE), followedShowId: 7n };
  requests = [season3];
  events.send({ event: { case: "requestCreated", value: { request: season3 } } });

  await expect(within(dark).findByText("Season 3")).resolves.toBeDefined();
  expect(within(dark).getByText("Available")).toBeDefined();
});

test("a Pending Followed Show approved elsewhere shows as Approved without a reload", async () => {
  const events = eventFeed();
  let followed: MessageInitShape<typeof FollowedShowSchema> = {
    ...followedDark,
    state: RequestState.PENDING,
  };
  openApp(
    "/?settings=requests",
    server(
      () => [],
      events.stream,
      () => [followed],
    ),
  );
  await screen.findByText("Pending");

  followed = followedDark;
  events.send({ event: { case: "followedShowStateChanged", value: { followedShow: followed } } });

  await expect(screen.findByText("Approved")).resolves.toBeDefined();
});

test("the Requests page says so when there are none", async () => {
  openApp(
    "/?settings=requests",
    server(() => []),
  );

  await expect(
    screen.findByText("No Requests yet. Request a Movie or Show from its page."),
  ).resolves.toBeDefined();
});

test("a Request made elsewhere shows up without a reload", async () => {
  const events = eventFeed();
  let requests: MessageInitShape<typeof RequestSchema>[] = [];
  openApp(
    "/?settings=requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("No Requests yet. Request a Movie or Show from its page.");

  requests = [dune];
  events.send({ event: { case: "requestCreated", value: { request: dune } } });

  await expect(screen.findByText("Dune")).resolves.toBeDefined();
});

test.each([
  { how: "approved elsewhere", state: RequestState.APPROVED, label: "Approved" },
  { how: "taken by its Fulfiller", state: RequestState.PROCESSING, label: "Processing" },
  { how: "Matched by a scan", state: RequestState.AVAILABLE, label: "Available" },
])("a Request $how shows as $label without a reload", async ({ state, label }) => {
  const events = eventFeed();
  let requests: MessageInitShape<typeof RequestSchema>[] = [heat];
  openApp(
    "/?settings=requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("Pending");

  const changed = { ...heat, state };
  requests = [changed];
  events.send({ event: { case: "requestStateChanged", value: { request: changed } } });

  await expect(screen.findByText(label)).resolves.toBeDefined();
});

test("a Request its Fulfiller didn't take shows as Failed with the reason, without a reload", async () => {
  const events = eventFeed();
  let requests: MessageInitShape<typeof RequestSchema>[] = [dune];
  openApp(
    "/?settings=requests",
    server(() => requests, events.stream),
  );
  await screen.findByText("Approved");

  const failed = { ...dune, state: RequestState.FAILED, failureReason: "Radarr is unreachable" };
  requests = [failed];
  events.send({ event: { case: "requestStateChanged", value: { request: failed } } });

  await expect(screen.findByText("Failed")).resolves.toBeDefined();
  expect(screen.getByText("Reason: Radarr is unreachable")).toBeDefined();
});

test("a Season Request shows as Partially Available, then Available, as a scan Matches its Episodes, without a reload", async () => {
  const events = eventFeed();
  const season2 = darkSeason(4n, 2, RequestState.PENDING);
  let requests = [season2, darkSeason(3n, 1, RequestState.PROCESSING)];
  openApp(
    "/?settings=requests",
    server(() => requests, events.stream),
  );
  const seasons = await screen.findByRole("list", { name: "Dark" });

  async function season1Becomes(state: RequestState, label: string) {
    const season1 = darkSeason(3n, 1, state);
    requests = [season2, season1];
    events.send({ event: { case: "requestStateChanged", value: { request: season1 } } });
    await within(seasons).findByText(label);
    return within(seasons)
      .getAllByRole("listitem")
      .map((row) => row.textContent);
  }

  const someMatched = await season1Becomes(RequestState.PARTIALLY_AVAILABLE, "Partially Available");
  const allMatched = await season1Becomes(RequestState.AVAILABLE, "Available");

  expect(someMatched).toStrictEqual([
    "Season 1Partially Available2017 · Requested by you",
    "Season 2Pending2017 · Requested by you",
  ]);
  expect(allMatched).toStrictEqual([
    "Season 1Available2017 · Requested by you",
    "Season 2Pending2017 · Requested by you",
  ]);
});

test.each([
  { how: "cancelled", request: dune, removal: "requestCancelled" },
  { how: "withdrawn from", request: heat, removal: "requestWithdrawn" },
] as const)("a Request $how elsewhere goes without a reload", async ({ request, removal }) => {
  const events = eventFeed();
  let requests = [request];
  openApp(
    "/?settings=requests",
    server(() => requests, events.stream),
  );
  await screen.findByText(request.movieOrSeason.value.title);

  requests = [];
  events.send({ event: { case: removal, value: { request } } });

  await expect(
    screen.findByText("No Requests yet. Request a Movie or Show from its page."),
  ).resolves.toBeDefined();
});

test("a Declined Request shows its Requesters why", async () => {
  openApp(
    "/?settings=requests",
    server(() => [{ ...heat, state: RequestState.DECLINED, declineReason: "Not out on disc yet" }]),
  );

  await expect(screen.findByText("Declined")).resolves.toBeDefined();
  expect(screen.getByText("Reason: Not out on disc yet")).toBeDefined();
});
