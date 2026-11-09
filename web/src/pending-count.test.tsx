import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { EventSchema } from "@/gen/nama/v1/event_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { eventFeed, fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** A fake Server signed in with `permissions`, whose Pending count is whatever `pending` returns. */
function server(permissions: Permission[], pending: () => number, events = eventFeed()) {
  return fakeServer({ signedIn: permissions, events: events.stream }, {}, {}, (router) => {
    router.service(RequestService, { countPendingRequests: () => ({ count: pending() }) });
  });
}

const manager = [...newUser, Permission.MANAGE_REQUESTS];

test.each<{
  what: string;
  event: MessageInitShape<typeof EventSchema>;
  to: number;
  name: string;
  text: string;
}>([
  {
    what: "a Request approved",
    event: { event: { case: "requestStateChanged", value: {} } },
    to: 0,
    name: "Requests",
    text: "Requests",
  },
  {
    what: "a Followed Show opened by someone else",
    event: { event: { case: "followedShowCreated", value: {} } },
    to: 5,
    name: "Requests, 5 Pending",
    text: "Requests5",
  },
])(
  "the Pending count on the Requests link follows $what without a reload",
  async ({ event, to, name, text }) => {
    const events = eventFeed();
    let pending = 1;
    openApp(
      "/settings",
      server(manager, () => pending, events),
    );

    const link = await screen.findByRole("link", { name: "Requests, 1 Pending" });
    expect(link.textContent).toBe("Requests1");
    pending = to;
    events.send(event);

    expect(await screen.findByRole("link", { name })).toBe(link);
    expect(link.textContent).toBe(text);
  },
);

test("the Requests link reads 99+ above 99 Pending, and its name gives the count", async () => {
  openApp(
    "/settings",
    server(manager, () => 150),
  );

  const link = await screen.findByRole("link", { name: "Requests, 150 Pending" });
  expect(link.textContent).toBe("Requests99+");
});

test("a User without Manage Requests sees no Pending count and never asks for one", async () => {
  let asked = false;
  openApp(
    "/settings",
    server(newUser, () => {
      asked = true;
      return 3;
    }),
  );

  await screen.findByRole("heading", { name: "Settings" });
  expect(screen.getByRole("link", { name: "Requests" }).textContent).toBe("Requests");
  expect(asked).toBe(false);
});
