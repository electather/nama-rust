import { type MessageInitShape, create } from "@bufbuild/protobuf";
import {
  Code,
  type ConnectRouter,
  ConnectError,
  type ServiceImpl,
  createRouterTransport,
} from "@connectrpc/connect";
import { RouterProvider } from "@tanstack/react-router";
import { configure, fireEvent, render, screen } from "@testing-library/react";

import {
  EventSchema,
  EventService,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { LibraryService, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { Permission, SignInService, WhoAmIResponseSchema } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

import { getRouter } from "./router";

// A file's first `findBy…` waits while the app's route chunks load for the first time, which took
// over the default 1 s on a busy CI runner.
configure({ asyncUtilTimeout: 5000 });

/** What a new User gets: the Permissions to make Requests, which manage nothing. */
export const newUser = [Permission.REQUEST, Permission.VIEW_REQUESTS];

/** What the first Admin has. */
export const admin = [Permission.ADMIN];

/** What the fake Server says: whether it needs setting up, who is signed in, if anyone, and its Libraries. */
export interface ServerState {
  setupRequired?: boolean;
  /** The Permissions of the User signed in, if anyone is. */
  signedIn?: Permission[];
  libraries?: MessageInitShape<typeof LibrarySchema>[];
  /** The Events the app's subscription gets. Without them, none arrive. */
  events?: ReadableStream<SubscribeResponse>;
  /** How the fake Server answers `ServerService` RPCs, over its defaults: its info, and no settings set. */
  serverService?: Partial<ServiceImpl<typeof ServerService>>;
}

/**
 * A fake Server in `state`, with handlers for the rest of `SignInService` and `UserService`.
 * `more` adds other services, such as `BrowseService`.
 */
export function fakeServer(
  state: ServerState,
  signIn: Partial<ServiceImpl<typeof SignInService>> = {},
  users: Partial<ServiceImpl<typeof UserService>> = {},
  more: (router: ConnectRouter) => void = () => {},
) {
  return createRouterTransport((router) => {
    router.service(ServerService, {
      getServerInfo: () =>
        create(GetServerInfoResponseSchema, {
          name: "Living Room",
          setupRequired: state.setupRequired ?? false,
        }),
      getServerSettings: () => ({ settings: {} }),
      ...state.serverService,
    });
    router.service(SignInService, {
      whoAmI: () => {
        if (state.signedIn === undefined) {
          throw new ConnectError("signed out", Code.Unauthenticated);
        }
        return create(WhoAmIResponseSchema, {
          user: { id: 1n, name: "Sam", permissions: state.signedIn },
        });
      },
      ...signIn,
    });
    // Settings lists your Devices and Plugin Connections, so every fake Server has an answer for them.
    // The Users page shows the default Permissions, so every fake Server has them.
    router.service(UserService, {
      listDevices: () => ({}),
      listPluginConnections: () => ({}),
      getDefaultPermissions: () => ({ permissions: newUser }),
      ...users,
    });
    router.service(LibraryService, { listLibraries: () => ({ libraries: state.libraries ?? [] }) });
    // Every signed-in page follows the Server's Events.
    router.service(EventService, { subscribe: () => state.events ?? new ReadableStream() });
    more(router);
  });
}

/** The title of each row on the page, Show titles included, top to bottom. */
export function rowTitles() {
  // Each row's title is its first paragraph.
  return screen.getAllByRole("listitem").map((row) => row.querySelector("p")?.textContent);
}

/** Opens the app at `path`, talking to the fake Server behind `transport`. */
export function openApp(path: string, transport: ReturnType<typeof createRouterTransport>) {
  window.history.replaceState(null, "", path);
  const router = getRouter(transport);
  // A route's error page shows what it caught. React's own log of it would inspect the error's
  // `Headers`, which happy-dom can't. The shell renders `<html>` and `<body>`, so the app takes the
  // whole document, as in a browser: inside a `<div>` in the body React adopts, the first `focus()`
  // (a menu handing focus back) spins forever finding the event's root.
  render(<RouterProvider router={router} />, { container: document, onCaughtError: () => {} });
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
