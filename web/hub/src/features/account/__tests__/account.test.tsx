import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

/** A Connect error as the Hub refuses with one. */
function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason }) },
  ]);
}

/**
 * Opens `path` on a fake Hub that has Ada's Account, with the password "correct horse", and
 * signs in or out as asked. Ada is signed in from the start when `signedIn` says so.
 */
function openHub(path: string, signedIn = false) {
  let session = signedIn;
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { account: ada };
      },
      signIn: ({ email, password }) => {
        if (email !== ada.email || password !== "correct horse") {
          throw refused(Code.Unauthenticated, RefusalReason.WRONG_EMAIL_OR_PASSWORD);
        }
        session = true;
        return { account: ada };
      },
      createAccount: ({ email }) => {
        if (email === ada.email) {
          throw refused(Code.AlreadyExists, RefusalReason.EMAIL_TAKEN);
        }
        session = true;
        return { account: ada };
      },
      signOut: () => {
        if (!session) {
          throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        session = false;
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", path);
  render(<RouterProvider router={getRouter(transport)} />);
  return {
    /** Ends Ada's session on the Hub, as if it expired, without the page knowing. */
    endSession: () => {
      session = false;
    },
  };
}

function fillIn(email: string, password: string) {
  fireEvent.change(screen.getByLabelText("Email"), { target: { value: email } });
  fireEvent.change(screen.getByLabelText("Password"), { target: { value: password } });
}

describe("an Account", () => {
  beforeAll(() => {
    // A file's first `findBy…` waits while the route chunks load for the first time.
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("signed out, the Account page sends you to sign in", async () => {
    openHub("/");

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
  });

  test("signing in opens the Account page, with the email and a sign-out button", async () => {
    openHub("/sign-in");
    await screen.findByRole("heading", { name: "Sign in" });

    fillIn("ada@example.com", "correct horse");
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    await expect(screen.findByRole("heading", { name: "Your Account" })).resolves.toBeDefined();
    expect(screen.getByText("ada@example.com")).toBeDefined();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeDefined();
  });

  test("a wrong password says the email and password don’t match, under the password", async () => {
    openHub("/sign-in");
    await screen.findByRole("heading", { name: "Sign in" });

    fillIn("ada@example.com", "wrong horse");
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    const error = await screen.findByText("That email and password don’t match an Account.");
    const password = screen.getByLabelText("Password");
    expect(password.getAttribute("aria-invalid")).toBe("true");
    expect(password.getAttribute("aria-describedby")).toBe(error.id);
  });

  test.each([
    { path: "/sign-in", heading: "Sign in" },
    { path: "/create-account", heading: "Create an Account" },
  ])("$heading marks the email for password managers to pair", async ({ path, heading }) => {
    openHub(path);
    await screen.findByRole("heading", { name: heading });

    const email = screen.getByLabelText("Email");
    expect({
      autocomplete: email.getAttribute("autocomplete"),
      type: email.getAttribute("type"),
    }).toStrictEqual({ autocomplete: "username", type: "email" });
  });

  test("creating an Account opens its Account page", async () => {
    openHub("/create-account");
    await screen.findByRole("heading", { name: "Create an Account" });

    fillIn("bea@example.com", "correct horse");
    fireEvent.click(screen.getByRole("button", { name: "Create Account" }));

    await expect(screen.findByRole("heading", { name: "Your Account" })).resolves.toBeDefined();
  });

  test("an email an Account already has is refused under the email", async () => {
    openHub("/create-account");
    await screen.findByRole("heading", { name: "Create an Account" });

    fillIn("ada@example.com", "correct horse");
    fireEvent.click(screen.getByRole("button", { name: "Create Account" }));

    await expect(
      screen.findByText("An Account already has this email. Sign in instead."),
    ).resolves.toBeDefined();
    expect(screen.getByLabelText("Email").getAttribute("aria-invalid")).toBe("true");
  });

  test("signing out goes back to the sign-in page", async () => {
    openHub("/", true);

    fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
  });

  test("signing out of a session that already ended goes to the sign-in page too", async () => {
    const hub = openHub("/", true);
    const signOut = await screen.findByRole("button", { name: "Sign out" });

    hub.endSession();
    fireEvent.click(signOut);

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
  });
});
