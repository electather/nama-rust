import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, fill, newUser, openApp, type ServerState } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

/** A signed-out Server whose `SignIn` signs the browser in as a new User. */
function serverToSignInTo() {
  const state: ServerState = {};
  return fakeServer(state, {
    signIn: () => {
      state.signedIn = newUser;
      return {};
    },
  });
}

/** A fake Server whose `SignIn` always fails with `code`, for `reason`. */
function serverRefusingSignIn(code: Code, reason: RefusalReason) {
  return fakeServer(
    {},
    {
      signIn: () => {
        throw refused(code, "refused", { reason });
      },
    },
  );
}

async function submitSignIn(password = "correct horse") {
  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  fill("Username", "Sam");
  fill("Password", password);
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
}

async function submitSetup(setupCode: string) {
  await screen.findByRole("heading", { name: "Set up Living Room" });
  fill("Setup Code", setupCode);
  fill("Username", "Sam");
  fill("Password", "correct horse");
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
      state.signedIn = admin;
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
        throw refused(Code.PermissionDenied, "wrong Setup Code", {
          reason: RefusalReason.WRONG_SETUP_CODE,
          field: "setup_code",
        });
      },
    },
  );
  openApp("/setup", transport);

  await submitSetup("WRONG");
  await expectUnderField("Setup Code", "That isn’t the Setup Code. Check the Server’s log for it.");
});

/** A fresh Server whose `Setup` refuses the request field `field` with `message`. */
function serverRefusingSetupField(field: string, message: string) {
  return fakeServer(
    { setupRequired: true },
    {
      setup: () => {
        throw refused(Code.InvalidArgument, message, { field });
      },
    },
  );
}

test.each([
  [
    "name",
    "Username",
    "a User's name needs 1 to 64 characters",
    "A User's name needs 1 to 64 characters.",
  ],
  [
    "password",
    "Password",
    "the password needs at least 8 characters",
    "The password needs at least 8 characters.",
  ],
])("a refused %s on Setup is shown under its field", async (field, label, message, shown) => {
  openApp("/setup", serverRefusingSetupField(field, message));

  await submitSetup("ABCD-EFGH");
  await expectUnderField(label, shown);
});

test("a refused Device name on Setup isn't shown under the name or password", async () => {
  openApp(
    "/setup",
    serverRefusingSetupField("device_name", "a Device's name needs 1 to 64 characters"),
  );

  await submitSetup("ABCD-EFGH");
  await screen.findByText("A Device's name needs 1 to 64 characters.");
  expect(screen.getByLabelText("Username").getAttribute("aria-invalid")).toBe("false");
  expect(screen.getByLabelText("Password").getAttribute("aria-invalid")).toBe("false");
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

test("a User who manages nothing opening an Admin page is told they can't, and has no Admin link", async () => {
  openApp("/admin/users", fakeServer({ signedIn: newUser }));

  expect(await screen.findByRole("heading", { name: "You can’t open this page" })).toBeDefined();
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test.each([
  [
    "Manage Libraries",
    Permission.MANAGE_LIBRARIES,
    "/admin/libraries",
    ["Libraries", "Needs a Match"],
  ],
  ["Manage Users", Permission.MANAGE_USERS, "/admin/users", ["Users"]],
  ["Manage settings", Permission.MANAGE_SETTINGS, "/admin/plugins", ["Plugins"]],
  [
    "Admin",
    Permission.ADMIN,
    "/admin/libraries",
    ["Libraries", "Needs a Match", "Users", "Plugins"],
  ],
])(
  "with only the %s Permission, Admin opens on %s and lists only the pages it allows",
  async (_, permission, landing, pages) => {
    const router = openApp(
      "/admin",
      fakeServer({ signedIn: [permission] }, {}, { listUsers: () => ({}) }),
    );

    const nav = await screen.findByRole("navigation", { name: "Admin" });
    expect(
      within(nav)
        .getAllByRole("link")
        .map((link) => link.textContent),
    ).toEqual(pages);
    expect(router.state.location.pathname).toBe(landing);
  },
);

test.each(["/admin/users", "/admin/plugins", "/admin/plugins/tmdb"])(
  "a User with only Manage Libraries opening %s is told they can't",
  async (path) => {
    openApp(path, fakeServer({ signedIn: [Permission.MANAGE_LIBRARIES] }));

    expect(await screen.findByRole("heading", { name: "You can’t open this page" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Admin" })).toBeDefined();
  },
);

test("a wrong password shows an error and keeps the username", async () => {
  const router = openApp(
    "/sign-in",
    serverRefusingSignIn(Code.Unauthenticated, RefusalReason.WRONG_NAME_OR_PASSWORD),
  );

  await submitSignIn("wrong");
  expect(await screen.findByText("That username and password don’t match.")).toBeDefined();
  expect(screen.getByLabelText<HTMLInputElement>("Username").value).toBe("Sam");
  expect(router.state.location.pathname).toBe("/sign-in");
});

test("a rate-limited sign-in says to try again in a moment", async () => {
  openApp(
    "/sign-in",
    serverRefusingSignIn(Code.ResourceExhausted, RefusalReason.TOO_MANY_ATTEMPTS),
  );

  await submitSignIn();
  expect(await screen.findByText(/try again in a moment/iu)).toBeDefined();
});

test("signing out goes to /sign-in", async () => {
  const state: ServerState = { signedIn: newUser };
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
  const state: ServerState = { signedIn: newUser };
  const transport = fakeServer(state, {
    // The Server revoked this Device after the page loaded.
    signOut: () => {
      throw refused(Code.Unauthenticated, "signed out", { reason: RefusalReason.SIGN_IN_REQUIRED });
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
      { signedIn: newUser },
      {
        signOut: () => {
          throw refused(Code.Unauthenticated, "refused", {
            reason: RefusalReason.SIGN_IN_REQUIRED,
          });
        },
      },
    ),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
  await screen.findByRole("heading", { name: "Settings" });
  expect(router.state.location.pathname).toBe("/settings");
});
