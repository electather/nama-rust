import { type MessageInitShape, create } from "@bufbuild/protobuf";
import {
  Code,
  type ConnectRouter,
  ConnectError,
  type ServiceImpl,
  createRouterTransport,
} from "@connectrpc/connect";
import { RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen } from "@testing-library/react";

import { EventService, type SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryService, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { Role, SignInService, WhoAmIResponseSchema } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

import { getRouter } from "./router";

/** What the fake Server says: whether it needs setting up, who is signed in, if anyone, and its Libraries. */
export interface ServerState {
  setupRequired?: boolean;
  signedIn?: Role;
  libraries?: MessageInitShape<typeof LibrarySchema>[];
  /** The Events the app's subscription gets. Without them, none arrive. */
  events?: ReadableStream<SubscribeResponse>;
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
    });
    router.service(SignInService, {
      whoAmI: () => {
        if (state.signedIn === undefined) {
          throw new ConnectError("signed out", Code.Unauthenticated);
        }
        return create(WhoAmIResponseSchema, {
          user: { id: 1n, name: "Sam", role: state.signedIn },
        });
      },
      ...signIn,
    });
    // Settings lists your Devices, so every fake Server has an answer for it.
    router.service(UserService, { listDevices: () => ({}), ...users });
    router.service(LibraryService, { listLibraries: () => ({ libraries: state.libraries ?? [] }) });
    // Every signed-in page follows the Server's Events.
    router.service(EventService, { subscribe: () => state.events ?? new ReadableStream() });
    more(router);
  });
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
