import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, fill, newUser, openApp, type ServerState } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";

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
  fill("1. Setup Code", setupCode);
  fill("Username", "Sam");
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Set up" }));
}

test.each(["/", "/libraries/1", "/sign-in"])("a fresh Server sends %s to /setup", async (path) => {
  const router = openApp(path, fakeServer({ setupRequired: true }));

  await expect(screen.findByRole("heading", { name: "Set up Living Room" })).resolves.toBeDefined();
  expect(router.state.location.pathname).toBe("/setup");
});

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
  await expect(screen.findByRole("button", { name: /’s menu$/u })).resolves.toBeDefined();
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
  await expectUnderField(
    "1. Setup Code",
    "That isn’t the Setup Code. Check the Server’s log for it.",
  );
});

test("setting up a Server someone else just set up offers to sign in instead", async () => {
  const transport = fakeServer(
    { setupRequired: true },
    {
      setup: () => {
        throw refused(Code.FailedPrecondition, "already set up", {
          reason: RefusalReason.ALREADY_SET_UP,
        });
      },
    },
  );
  openApp("/setup", transport);

  await submitSetup("ABCD-EFGH");
  await expect(
    screen.findByText("This Server is already set up, so you can sign in."),
  ).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Go to sign in" }).getAttribute("href")).toBe("/sign-in");
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
  {
    field: "name",
    label: "Username",
    message: "a User's name needs 1 to 64 characters",
    shown: "A User's name needs 1 to 64 characters.",
  },
  {
    field: "password",
    label: "Password",
    message: "the password needs at least 8 characters",
    shown: "The password needs at least 8 characters.",
  },
])(
  "a refused $field on Setup is shown under its field",
  async ({ field, label, message, shown }) => {
    openApp("/setup", serverRefusingSetupField(field, message));

    await submitSetup("ABCD-EFGH");
    await expectUnderField(label, shown);
  },
);

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

/** Signs out from your menu in the top bar, once the app shows it. */
async function signOut() {
  fireEvent.click(await screen.findByRole("button", { name: /’s menu$/u }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Sign out" }));
}

test("signed out, a page sends you to sign in and back again", async () => {
  const router = openApp("/libraries/1", serverToSignInTo());

  await submitSignIn();
  expect({ ...router.state.location.search }).toStrictEqual({ redirect: "/libraries/1" });
  await screen.findByRole("button", { name: /’s menu$/u });
  expect(router.state.location.pathname).toBe("/libraries/1");
});

test("signing in never redirects to another site", async () => {
  const router = openApp("/sign-in?redirect=https%3A%2F%2Fevil.example%2F", serverToSignInTo());

  await submitSignIn();
  await screen.findByRole("button", { name: /’s menu$/u });
  expect(router.state.location.href).toBe("/");
});

test("a User who manages nothing has no Admin link, and /admin is a page not found", async () => {
  openApp("/admin", fakeServer({ signedIn: newUser }));

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("a Library manager's Admin link opens Settings at Libraries, and Back closes it", async () => {
  const router = openApp("/requests", fakeServer({ signedIn: [Permission.MANAGE_LIBRARIES] }));

  fireEvent.click(await screen.findByRole("link", { name: "Admin" }));

  await screen.findByRole("dialog", { name: "Settings" });
  expect(router.state.location.search.settings).toBe("libraries");
  router.history.back();
  await waitFor(() => {
    expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull();
  });
});

test.each([
  { who: "an Admin", permissions: admin, section: "libraries" },
  { who: "a Settings manager", permissions: [Permission.MANAGE_SETTINGS], section: "server" },
  { who: "a User manager", permissions: [Permission.MANAGE_USERS], section: "users" },
])(
  "$who's G then A opens Settings at $section, over the page",
  async ({ permissions, section }) => {
    const router = openApp("/requests", fakeServer({ signedIn: permissions }));
    await screen.findByRole("link", { name: "Admin" });

    fireEvent.keyDown(document.body, { key: "g" });
    fireEvent.keyDown(document.body, { key: "a" });

    await screen.findByRole("dialog", { name: "Settings" });
    expect(router.state.location.pathname).toBe("/requests");
    expect(router.state.location.search.settings).toBe(section);
  },
);

test("a wrong password shows an error and keeps the username", async () => {
  const router = openApp(
    "/sign-in",
    serverRefusingSignIn(Code.Unauthenticated, RefusalReason.WRONG_NAME_OR_PASSWORD),
  );

  await submitSignIn("wrong");
  await expect(screen.findByText("That username and password don’t match.")).resolves.toBeDefined();
  expect(screen.getByLabelText<HTMLInputElement>("Username").value).toBe("Sam");
  expect(router.state.location.pathname).toBe("/sign-in");
});

test("a rate-limited sign-in says to wait, or sign in with a code", async () => {
  openApp(
    "/sign-in",
    serverRefusingSignIn(Code.ResourceExhausted, RefusalReason.TOO_MANY_ATTEMPTS),
  );

  await submitSignIn();
  await expect(
    screen.findByText(
      "There have been too many tries. Try again in a few minutes, or sign in with a code.",
    ),
  ).resolves.toBeDefined();
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

  await signOut();
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
  const router = openApp("/code", transport);
  await screen.findByRole("button", { name: /’s menu$/u });
  delete state.signedIn;

  await signOut();
  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  expect({ ...router.state.location.search }).toStrictEqual({ redirect: "/code" });
});

test("an `unauthenticated` refusal while still signed in keeps you on the page", async () => {
  const router = openApp(
    "/code",
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

  await signOut();
  await screen.findByRole("heading", { name: "Sign in a Device" });
  expect(router.state.location.pathname).toBe("/code");
});
