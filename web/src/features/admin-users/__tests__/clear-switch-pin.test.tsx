import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { ClearSwitchPinRequest } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, newUser, openApp } from "@/test-app";
import { refused } from "@/test-refusal";

afterEach(cleanup);

/**
 * Alex's page in Settings' Users, on a Server that refuses to clear a Switch PIN with `refusal`, or
 * else records whose it cleared.
 */
function openAlex(refusal?: Error) {
  const cleared: (bigint | undefined)[] = [];
  const clearSwitchPin = ({ userId }: ClearSwitchPinRequest) => {
    if (refusal !== undefined) {
      throw refusal;
    }
    cleared.push(userId);
    return {};
  };
  openApp(
    "/code?settings=users&user=2",
    fakeServer(
      { signedIn: admin },
      { clearSwitchPin },
      { listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }) },
    ),
  );
  return cleared;
}

/** Opens the dialog asking to clear Alex's Switch PIN. */
async function askToClear() {
  fireEvent.click(await screen.findByRole("button", { name: "Clear Switch PIN for Alex" }));
  return screen.findByRole("alertdialog", { name: "Clear Alex’s Switch PIN?" });
}

test("confirming clears that User's Switch PIN, and the dialog closes", async () => {
  const cleared = openAlex();

  const dialog = await askToClear();
  fireEvent.click(within(dialog).getByRole("button", { name: "Clear Switch PIN" }));

  await waitFor(() => {
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
  expect(cleared).toStrictEqual([2n]);
});

test("cancelling leaves the Switch PIN as it was", async () => {
  const cleared = openAlex();

  const dialog = await askToClear();
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

  await waitFor(() => {
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
  expect(cleared).toStrictEqual([]);
});

test.each([
  [
    "a User deleted meanwhile",
    refused(Code.NotFound, "no such User", { reason: RefusalReason.USER_NOT_FOUND }),
    "Alex isn’t on this Server any more.",
  ],
  [
    "an Admin's, for want of the Admin Permission",
    refused(Code.PermissionDenied, "needs PERMISSION_ADMIN", {
      reason: RefusalReason.PERMISSION_REQUIRED,
      params: { permission: "PERMISSION_ADMIN" },
    }),
    "Only an Admin can clear an Admin’s Switch PIN.",
  ],
])("clearing %s says why it was refused", async (_, refusal, shown) => {
  openAlex(refusal);

  const dialog = await askToClear();
  fireEvent.click(within(dialog).getByRole("button", { name: "Clear Switch PIN" }));

  await expect(within(dialog).findByText(shown)).resolves.toBeDefined();
});
