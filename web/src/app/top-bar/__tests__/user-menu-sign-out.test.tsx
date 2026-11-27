import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { fakeServer, newUser, openApp } from "@/test-app";
import { accountHub, ada } from "@/test-hub";
import { pickFromMenu } from "@/test-menu";

afterEach(cleanup);

/** What `fail` is until the Server is asked to sign the Device out. */
const unasked = (_error: Error) => {};

/** A fake Server whose `revokeDevice` fails, as when it can't be reached. */
function unreachableServer() {
  return fakeServer(
    { signedIn: newUser },
    {},
    {
      revokeDevice: () => {
        throw new ConnectError("unreachable", Code.Unavailable);
      },
    },
  );
}

test("at the Hub, a failed sign-out says so and leaves the Account signed in", async () => {
  openApp("/", accountHub({ signOutFails: true }).transport);

  await pickFromMenu(`${ada.email}’s menu`, "Sign out");

  await expect(screen.findByRole("alert")).resolves.toHaveProperty(
    "textContent",
    "The Hub couldn’t sign you out just now. Try again.",
  );
  expect(screen.getByText("Not signed out")).toBeDefined();
  expect(screen.getByRole("button", { name: `${ada.email}’s menu` })).toBeDefined();
});

test("at a Server, a failed sign-out says the Server couldn't", async () => {
  openApp("/", unreachableServer());

  await pickFromMenu("Sam’s menu", "Sign out");

  await expect(screen.findByRole("alert")).resolves.toHaveProperty(
    "textContent",
    "The Server couldn’t sign you out just now. Try again.",
  );
  expect(screen.getByText("Not signed out")).toBeDefined();
});

test("dismissing a failed sign-out hides it", async () => {
  openApp("/", unreachableServer());
  await pickFromMenu("Sam’s menu", "Sign out");

  fireEvent.click(await screen.findByRole("button", { name: "Dismiss" }));

  await waitFor(() => {
    expect(screen.queryByText("Not signed out")).toBeNull();
  });
});

test("signing out again clears the last failure", async () => {
  // Each sign-out waits until the test fails it: the second, never answered, clears it by starting.
  let fail = unasked;
  openApp(
    "/",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        revokeDevice: async () => {
          await new Promise((_resolve, reject) => {
            fail = reject;
          });
          return {};
        },
      },
    ),
  );
  await pickFromMenu("Sam’s menu", "Sign out");
  await waitFor(() => {
    expect(fail).not.toBe(unasked);
  });
  fail(new ConnectError("unreachable", Code.Unavailable));
  await screen.findByText("Not signed out");

  await pickFromMenu("Sam’s menu", "Sign out");

  await waitFor(() => {
    expect(screen.queryByText("Not signed out")).toBeNull();
  });
});

test("at the Hub, a sign-out refused for want of sign-in opens sign-in without a toast", async () => {
  const hub = accountHub();
  openApp("/", hub.transport);
  await screen.findByRole("button", { name: `${ada.email}’s menu` });
  // Signed out elsewhere after the page loaded, so signing out is refused.
  hub.hub.signedIn = false;

  await pickFromMenu(`${ada.email}’s menu`, "Sign out");

  await expect(screen.findByRole("heading", { name: "Sign in to the Hub" })).resolves.toBeDefined();
  expect(screen.queryByText("Not signed out")).toBeNull();
});
