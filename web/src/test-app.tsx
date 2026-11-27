import { type MessageInitShape, create } from "@bufbuild/protobuf";
import type { Transport } from "@connectrpc/connect";
import { RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen, within } from "@testing-library/react";

import {
  EventSchema,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import type { ServerAt } from "@/shared/lib/server";
import { tickWorker } from "@/test-worker";

import { getRouter } from "./router";

// Not `vi.stubGlobal`, which the files that call `vi.unstubAllGlobals` would undo after one test.
Object.assign(globalThis, { Worker: tickWorker });

/** The title of each row of Settings' Requests, Show titles included, top to bottom. */
export function requestTitles() {
  const list = screen.getByRole("list", { name: /^(?:Your|All) Requests$/u });
  // Each row's title is its link to the title's page, or a Season's name.
  return within(list)
    .getAllByRole("listitem")
    .map((row) => row.querySelector("a, p")?.textContent);
}

/**
 * Opens the app at `path`, talking to the fake Server behind `transport`, rendered inside `wrap`,
 * such as the `StrictMode` TanStack Start's client entry renders it in. At the Hub, `serverAt`
 * gives the fake Server at each origin.
 */
export function openApp(
  path: string,
  transport: Transport,
  wrap: (app: Parameters<typeof render>[0]) => Parameters<typeof render>[0] = (app) => app,
  serverAt?: ServerAt,
) {
  window.history.replaceState(null, "", path);
  const router = getRouter(transport, serverAt);
  // A route's error page shows what it caught. React's own log of it would inspect the error's
  // `Headers`, which happy-dom can't. The shell renders `<html>` and `<body>`, so the app takes the
  // whole document, as in a browser: inside a `<div>` in the body React adopts, the first `focus()`
  // (a menu handing focus back) spins forever finding the event's root.
  render(wrap(<RouterProvider router={router} />), {
    container: document,
    onCaughtError: () => {},
  });
  return router;
}

export function fill(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

/** Events for a test to send the app. What's sent before the app subscribes waits for it. */
export function eventFeed() {
  let feed: ReadableStreamDefaultController<SubscribeResponse> | undefined;
  return {
    stream: new ReadableStream<SubscribeResponse>({
      start: (controller) => {
        feed = controller;
      },
    }),
    send: (event: MessageInitShape<typeof EventSchema>) => {
      const value = create(EventSchema, event);
      feed?.enqueue(create(SubscribeResponseSchema, { message: { case: "event", value } }));
    },
  };
}
