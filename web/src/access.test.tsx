import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl, createRouterTransport } from "@connectrpc/connect";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { GetServerInfoResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { Role, SignInService, WhoAmIResponseSchema } from "@/gen/nama/v1/sign_in_pb";

import { getRouter } from "./router";

afterEach(cleanup);

/** What the fake Server says: whether it needs setting up, and who is signed in, if anyone. */
interface ServerState {
  setupRequired?: boolean;
  signedIn?: Role;
}

/** A fake Server in `state`, with `handlers` for the rest of `SignInService`. */
function fakeServer(state: ServerState, handlers: Partial<ServiceImpl<typeof SignInService>> = {}) {
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
      ...handlers,
    });
  });
}

/** A signed-out Server whose `SignIn` signs the browser in as a Member. */
function serverToSignInTo() {
  const state: ServerState = {};
  return fakeServer(state, {
    signIn: () => {
      state.signedIn = Role.MEMBER;
      return {};
    },
  });
}

/** A fake Server whose `SignIn` always fails with `code`. */
function serverRefusingSignIn(code: Code) {
  return fakeServer(
    {},
    {
      signIn: () => {
        throw new ConnectError("refused", code);
      },
    },
  );
}

function openApp(path: string, transport: ReturnType<typeof createRouterTransport>) {
  window.history.replaceState(null, "", path);
  const router = getRouter(transport);
  // A route's error page shows what it caught. React's own log of it would inspect the error's
  // `Headers`, which happy-dom can't.
  render(<RouterProvider router={router} />, { onCaughtError: () => {} });
  return router;
}

function fill(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

async function submitSignIn(password = "correct horse") {
  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  fill("Name", "Sam");
  fill("Password", password);
  fill("Device name", "Laptop");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
}

async function submitSetup(setupCode: string) {
  await screen.findByRole("heading", { name: "Set up Living Room" });
  fill("Setup Code", setupCode);
  fill("Your name", "Sam");
  fill("Password", "correct horse");
  fill("Device name", "Laptop");
  fireEvent.click(screen.getByRole("button", { name: "Set up" }));
}

test.each(["/", "/libraries/1", "/sign-in", "/admin/users"])(
  "a fresh Server sends %s to /setup",
  async (path) => {
    const router = openApp(path, fakeServer({ setupRequired: true }));

    expect(await screen.findByRole("heading", { name: "Set up Living Room" })).toBeDefined();
    expect(router.state.location.pathname).toBe("/setup");
  },
);

test("setting up creates the first Admin and lands on the home page", async () => {
  const state: ServerState = { setupRequired: true };
  const transport = fakeServer(state, {
    setup: () => {
      state.setupRequired = false;
      state.signedIn = Role.ADMIN;
      return {};
    },
  });
  const router = openApp("/setup", transport);

  await submitSetup("ABCD-EFGH");
  expect(await screen.findByRole("link", { name: "Admin" })).toBeDefined();
  expect(router.state.location.pathname).toBe("/");
});

test("a wrong Setup Code is shown next to its field", async () => {
  const transport = fakeServer(
    { setupRequired: true },
    {
      setup: () => {
        throw new ConnectError("wrong Setup Code", Code.PermissionDenied);
      },
    },
  );
  openApp("/setup", transport);

  await submitSetup("WRONG");
  const error = await screen.findByText(
    "That isn’t the Setup Code. Check the Server’s log for it.",
  );
  const field = screen.getByLabelText("Setup Code");
  expect(field.getAttribute("aria-invalid")).toBe("true");
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});

test("signed out, a page sends you to sign in and back again", async () => {
  const router = openApp("/libraries/1", serverToSignInTo());

  await submitSignIn();
  expect(router.state.location.search).toEqual({ redirect: "/libraries/1" });
  await screen.findByRole("button", { name: "Sign out" });
  expect(router.state.location.pathname).toBe("/libraries/1");
});

test("signing in never redirects to another site", async () => {
  const router = openApp("/sign-in?redirect=https%3A%2F%2Fevil.example%2F", serverToSignInTo());

  await submitSignIn();
  await screen.findByRole("button", { name: "Sign out" });
  expect(router.state.location.href).toBe("/");
});

test("a Member opening an Admin page sees Admins only, and no Admin link", async () => {
  openApp("/admin/users", fakeServer({ signedIn: Role.MEMBER }));

  expect(await screen.findByRole("heading", { name: "Admins only" })).toBeDefined();
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("a wrong password shows an error and keeps the name", async () => {
  const router = openApp("/sign-in", serverRefusingSignIn(Code.Unauthenticated));

  await submitSignIn("wrong");
  expect(await screen.findByText("That name and password don’t match.")).toBeDefined();
  expect(screen.getByLabelText<HTMLInputElement>("Name").value).toBe("Sam");
  expect(router.state.location.pathname).toBe("/sign-in");
});

test("a rate-limited sign-in says to try again in a moment", async () => {
  openApp("/sign-in", serverRefusingSignIn(Code.ResourceExhausted));

  await submitSignIn();
  expect(await screen.findByText(/try again in a moment/iu)).toBeDefined();
});

test("signing out goes to /sign-in", async () => {
  const state: ServerState = { signedIn: Role.MEMBER };
  const transport = fakeServer(state, {
    signOut: () => {
      delete state.signedIn;
      return {};
    },
  });
  const router = openApp("/", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  expect(router.state.location.pathname).toBe("/sign-in");
});

test("a session that has ended sends you to sign in and back again", async () => {
  const state: ServerState = { signedIn: Role.MEMBER };
  const transport = fakeServer(state, {
    // The Server revoked this Device after the page loaded.
    signOut: () => {
      throw new ConnectError("signed out", Code.Unauthenticated);
    },
  });
  const router = openApp("/settings", transport);
  const signOut = await screen.findByRole("button", { name: "Sign out" });
  delete state.signedIn;

  fireEvent.click(signOut);
  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  expect(router.state.location.search).toEqual({ redirect: "/settings" });
});

test("an `unauthenticated` refusal while still signed in keeps you on the page", async () => {
  const router = openApp(
    "/settings",
    fakeServer(
      { signedIn: Role.MEMBER },
      {
        signOut: () => {
          throw new ConnectError("refused", Code.Unauthenticated);
        },
      },
    ),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
  await screen.findByRole("heading", { name: "Settings" });
  expect(router.state.location.pathname).toBe("/settings");
});
