import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { HubService } from "@hub/gen/nama/hub/v1/hub_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

function refused(code: Code, reason: RefusalReason, params: Record<string, string> = {}) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason, params }) },
  ]);
}

/**
 * Opens the Account page on a fake Hub that emails Sign-in Links when `signInLinks` says so, where
 * Ada is signed in, with a password if `hasPassword` says so, and a sign-in in the last 10 minutes
 * if `recent` does. Returns the passwords set, whether the Hub has said if it emails links, and how
 * often the Account was asked for.
 */
function openAccountPage(hasPassword: boolean, recent = true, signInLinks = true) {
  let password = hasPassword;
  const set: string[] = [];
  let toldSignInLinks = false;
  let accountLookUps = 0;
  const transport = createRouterTransport(({ service }) => {
    service(HubService, {
      getHubInfo: () => {
        toldSignInLinks = true;
        return { version: "0.1.0", signInLinks };
      },
    });
    service(AccountService, {
      getAccount: () => {
        accountLookUps += 1;
        return { account: ada, hasPassword: password };
      },
      setPassword: (request) => {
        if (!recent) {
          throw refused(Code.FailedPrecondition, RefusalReason.RECENT_AUTHENTICATION_REQUIRED, {
            recentMinutes: "12",
          });
        }
        password = true;
        set.push(request.password);
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", "/");
  render(<RouterProvider router={getRouter(transport)} />);
  return { set, toldSignInLinks: () => toldSignInLinks, accountLookUps: () => accountLookUps };
}

async function submit(label: string, button: string) {
  fireEvent.change(await screen.findByLabelText(label), {
    target: { value: "staple battery horse" },
  });
  fireEvent.click(screen.getByRole("button", { name: button }));
}

describe("an Account's password", () => {
  beforeAll(() => {
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("the Account page asks for the Account once", async () => {
    const hub = openAccountPage(true);

    await expect(screen.findByRole("button", { name: "Change password" })).resolves.toBeDefined();
    expect(hub.accountLookUps()).toBe(1);
  });

  test("an Account without one sets one, after which it can change it", async () => {
    const { set } = openAccountPage(false);

    await submit("Password", "Set password");

    await expect(screen.findByText(/Your password is saved/u)).resolves.toBeDefined();
    await expect(screen.findByRole("button", { name: "Change password" })).resolves.toBeDefined();
    expect(set).toStrictEqual(["staple battery horse"]);
  });

  test("an Account with one changes it, signing out everywhere else", async () => {
    const { set } = openAccountPage(true);

    await submit("New password", "Change password");

    await expect(
      screen.findByText("Your password is saved, and you’re signed out everywhere else."),
    ).resolves.toBeDefined();
    expect(set).toStrictEqual(["staple battery horse"]);
  });

  test("the hint to set one mentions Sign-in Links only on a Hub that emails them", async () => {
    openAccountPage(false, true, true);
    await expect(
      screen.findByText(
        "8 to 1024 characters. You can sign in with it as well as with Sign-in Links.",
      ),
    ).resolves.toBeDefined();
    cleanup();

    const hub = openAccountPage(false, true, false);
    await expect(screen.findByText("8 to 1024 characters.")).resolves.toBeDefined();
    await waitFor(() => {
      expect(hub.toldSignInLinks()).toBe(true);
    });
    expect(screen.queryByText(/Sign-in Links/u)).toBeNull();
  });

  test("the hint to change one is the same whether the Hub emails Sign-in Links or not", async () => {
    const hint = "8 to 1024 characters. Changing it signs you out everywhere else.";
    // Setting one first: the change hint then only shows once the Hub's answer is on screen.
    openAccountPage(false, true, true);
    await expect(screen.findByText(/as well as with Sign-in Links/u)).resolves.toBeDefined();
    await submit("Password", "Set password");
    await expect(screen.findByText(hint)).resolves.toBeDefined();
    cleanup();

    openAccountPage(true, true, false);
    await expect(screen.findByText(hint)).resolves.toBeDefined();
  });

  test("a sign-in older than the Hub allows is sent to sign in again", async () => {
    const { set } = openAccountPage(true, false);

    await submit("New password", "Change password");

    const signInAgain = await screen.findByRole("link", { name: "Sign in again" });
    expect(screen.getByText(/You signed in over 12 minutes ago/u)).toBeDefined();
    expect(signInAgain.getAttribute("href")).toBe("/sign-in");
    expect(set).toStrictEqual([]);
  });
});
