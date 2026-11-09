import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { type RequestSchema, RequestService, RequestState } from "@/gen/nama/v1/request_pb";

import { eventFeed, fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** Heat, Pending, asked for by Sam, the signed-in User. */
const heat = {
  id: 2n,
  movie: { tmdbId: 949n, title: "Heat", releaseDate: "1995-12-15" },
  state: RequestState.PENDING,
  requesters: [{ userId: 1n, name: "Sam" }],
};

const darkSeason1 = {
  id: 3n,
  season: { showTmdbId: 70_523n, seasonNumber: 1, showTitle: "Dark", airDate: "2017-12-01" },
  state: RequestState.APPROVED,
  requesters: [{ userId: 1n, name: "Sam" }],
};

/** A fake Server whose `GetRequest` gives what `request` gives, or `not_found` for none. */
function server(
  request: () => MessageInitShape<typeof RequestSchema> | undefined,
  events = new ReadableStream<SubscribeResponse>(),
) {
  return fakeServer({ signedIn: newUser, events }, {}, {}, (router) => {
    router.service(RequestService, {
      getRequest: () => {
        const found = request();
        if (found === undefined) throw new ConnectError("no such Request", Code.NotFound);
        return { request: found };
      },
      listRequests: () => ({ requests: [heat] }),
    });
  });
}

const removed = "This Request was removed, or isn’t one you can see.";

test("a Request's page shows its row, with its actions, and a link to all Requests", async () => {
  openApp(
    "/requests/2",
    server(() => heat),
  );

  const row = await screen.findByRole("listitem");
  expect(row.textContent).toBe("Heat1995 · Requested by youPendingWithdraw");
  const all = screen.getByRole("link", { name: "All Requests" });
  expect(all.getAttribute("href")).toBe("/requests");
});

test("a Season Request's page shows it under its Show's title", async () => {
  openApp(
    "/requests/3",
    server(() => darkSeason1),
  );

  const dark = await screen.findByRole("list", { name: "Dark" });
  expect(within(dark).getByRole("listitem").textContent).toBe(
    "Season 12017 · Requested by youApproved",
  );
});

test.each(["/requests/9", "/requests/heat", "/requests/99999999999999999999"])(
  "%s, a Request that isn't there or isn't yours to see, says so",
  async (path) => {
    openApp(
      path,
      server(() => {}),
    );

    expect(await screen.findByText(removed)).toBeDefined();
    expect(screen.getByRole("link", { name: "All Requests" })).toBeDefined();
  },
);

test("a Request cancelled while its page is open says it was removed, without a reload", async () => {
  const events = eventFeed();
  let request: typeof heat | undefined = heat;
  openApp(
    "/requests/2",
    server(() => request, events.stream),
  );
  await screen.findByText("Heat");

  request = undefined;
  events.send({ event: { case: "requestCancelled", value: { request: heat } } });

  expect(await screen.findByText(removed)).toBeDefined();
  expect(screen.queryByText("Heat")).toBeNull();
});

test("a link Telegram sent to a Request's row lands on the Request's page", async () => {
  const router = openApp(
    "/requests#request-2",
    server(() => heat),
  );

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/requests/2");
  });
  expect(await screen.findByText("Heat")).toBeDefined();
});
