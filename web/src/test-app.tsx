import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl, createRouterTransport } from "@connectrpc/connect";
import { RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen } from "@testing-library/react";

import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { Role, SignInService, WhoAmIResponseSchema } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

import { getRouter } from "./router";

/** What the fake Server says: whether it needs setting up, and who is signed in, if anyone. */
export interface ServerState {
  setupRequired?: boolean;
  signedIn?: Role;
}

/** A fake Server in `state`, with handlers for the rest of `SignInService` and `UserService`. */
export function fakeServer(
  state: ServerState,
  signIn: Partial<ServiceImpl<typeof SignInService>> = {},
  users: Partial<ServiceImpl<typeof UserService>> = {},
) {
  return createRouterTransport(({ service }) => {
    service(ServerService, {
      getServerInfo: () =>
        create(GetServerInfoResponseSchema, {
          name: "Living Room",
          setupRequired: state.setupRequired ?? false,
        }),
    });
    service(SignInService, {
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
    service(UserService, { listDevices: () => ({}), ...users });
  });
}

/** Opens the app at `path`, talking to the fake Server behind `transport`. */
export function openApp(path: string, transport: ReturnType<typeof createRouterTransport>) {
  window.history.replaceState(null, "", path);
  const router = getRouter(transport);
  // A route's error page shows what it caught. React's own log of it would inspect the error's
  // `Headers`, which happy-dom can't.
  render(<RouterProvider router={router} />, { onCaughtError: () => {} });
  return router;
}

/** Types `value` into the field labelled `label`. */
export function fill(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}
