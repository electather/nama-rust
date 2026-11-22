import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason }) },
  ]);
}

/**
 * Opens the Account page on a fake Hub where Ada is signed in, with a password if `hasPassword`
 * says so, and a sign-in in the last 10 minutes if `recent` does. Returns the passwords set.
 */
function openAccountPage(hasPassword: boolean, recent = true) {
  let password = hasPassword;
  const set: string[] = [];
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => ({ account: ada, hasPassword: password }),
      setPassword: (request) => {
        if (!recent) {
          throw refused(Code.FailedPrecondition, RefusalReason.RECENT_AUTHENTICATION_REQUIRED);
        }
        password = true;
        set.push(request.password);
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", "/");
  render(<RouterProvider router={getRouter(transport)} />);
  return set;
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

  test("an Account without one sets one, after which it can change it", async () => {
    const set = openAccountPage(false);

    await submit("Password", "Set password");

    await expect(screen.findByText(/Your password is saved/u)).resolves.toBeDefined();
    await expect(screen.findByRole("button", { name: "Change password" })).resolves.toBeDefined();
    expect(set).toStrictEqual(["staple battery horse"]);
  });

  test("an Account with one changes it, signing out everywhere else", async () => {
    const set = openAccountPage(true);

    await submit("New password", "Change password");

    await expect(
      screen.findByText("Your password is saved, and you’re signed out everywhere else."),
    ).resolves.toBeDefined();
    expect(set).toStrictEqual(["staple battery horse"]);
  });

  test("a sign-in over 10 minutes old is sent to sign in again", async () => {
    const set = openAccountPage(true, false);

    await submit("New password", "Change password");

    const signInAgain = await screen.findByRole("link", { name: "Sign in again" });
    expect(signInAgain.getAttribute("href")).toBe("/sign-in");
    expect(set).toStrictEqual([]);
  });
});
