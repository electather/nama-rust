import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

/**
 * Opens the Account page on a fake Hub where Ada is signed in, with a sign-in in the last 10
 * minutes if `recent` says so. Returns how many times the Account was deleted.
 */
function openAccountPage(recent: boolean) {
  const deleted = { times: 0 };
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => ({ account: ada, hasPassword: true }),
      deleteAccount: () => {
        if (!recent) {
          throw new ConnectError("refused", Code.FailedPrecondition, undefined, [
            {
              desc: RefusalSchema,
              value: create(RefusalSchema, {
                reason: RefusalReason.RECENT_AUTHENTICATION_REQUIRED,
                params: { recentMinutes: "10" },
              }),
            },
          ]);
        }
        deleted.times += 1;
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", "/");
  const router = getRouter(transport);
  render(<RouterProvider router={router} />);
  return { deleted, router };
}

/** Opens Delete Account's dialog, and confirms in it. */
async function deleteAndConfirm() {
  fireEvent.click(await screen.findByRole("button", { name: "Delete Account" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Delete your Account?" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete Account" }));
  return dialog;
}

describe("deleting an Account", () => {
  beforeAll(() => {
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("once confirmed, it's deleted and the sign-in page opens", async () => {
    const { deleted, router } = openAccountPage(true);

    await deleteAndConfirm();

    await expect(screen.findByRole("heading", { name: /Sign in/u })).resolves.toBeDefined();
    expect(router.state.location.pathname).toBe("/sign-in");
    expect(deleted.times).toBe(1);
  });

  test("cancelling deletes nothing", async () => {
    const { deleted } = openAccountPage(true);

    fireEvent.click(await screen.findByRole("button", { name: "Delete Account" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete your Account?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(deleted.times).toBe(0);
  });

  test("a sign-in over 10 minutes ago is asked for again", async () => {
    const { deleted } = openAccountPage(false);

    const dialog = await deleteAndConfirm();

    await expect(
      within(dialog).findByText(/You signed in over 10 minutes ago/u),
    ).resolves.toBeDefined();
    expect(within(dialog).getByRole("link", { name: "Sign in again" })).toBeDefined();
    expect(deleted.times).toBe(0);
  });
});
