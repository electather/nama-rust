import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, fill, newUser, openApp } from "@/test-app";
import { confirmItsYou, recentAuthenticationRequired, refused } from "@/test-refusal";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  sessionStorage.clear();
});

async function submitPasswordChange() {
  await screen.findByRole("button", { name: "Change password" });
  fill("Current password", "correct horse");
  fill("New password", "battery staple");
  fill("Confirm new password", "battery staple");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));
}

/**
 * Settings' Sign-in pane, on a Server whose Recent Authentication has lapsed again by the time
 * `ChangePassword` arrives: the pane's own `Reauthenticate` isn't enough, so it refuses until a
 * second one gets "correct horse", or always if `alwaysRefuse`. Returns the passwords it changed to.
 */
function openSettingsNeedingRecentAuthentication(alwaysRefuse = false) {
  let confirmations = 0;
  const changedTo: string[] = [];
  openApp(
    "/code?settings=signin",
    fakeServer(
      { signedIn: newUser },
      {
        reauthenticate: ({ proof }) => {
          if (proof.case !== "password" || proof.value !== "correct horse") {
            throw refused(Code.Unauthenticated, "the password is wrong", {
              reason: RefusalReason.WRONG_PASSWORD,
              field: "password",
            });
          }
          confirmations += 1;
          return {};
        },
      },
      {
        changePassword: ({ newPassword }) => {
          if (alwaysRefuse || confirmations < 2) {
            throw recentAuthenticationRequired();
          }
          changedTo.push(newPassword);
          return {};
        },
      },
    ),
  );
  return changedTo;
}

test("without Recent Authentication, a password change goes through once you confirm it’s you", async () => {
  const changedTo = openSettingsNeedingRecentAuthentication();

  await submitPasswordChange();
  await confirmItsYou();

  await expect(screen.findByText("Password changed")).resolves.toBeDefined();
  expect(changedTo).toStrictEqual(["battery staple"]);
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("cancelling “Confirm it’s you” leaves the form as it was, without an error", async () => {
  const changedTo = openSettingsNeedingRecentAuthentication();

  await submitPasswordChange();
  const dialog = await screen.findByRole("alertdialog", { name: "Confirm it’s you" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

  await waitFor(() => {
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
  expect(screen.getByLabelText<HTMLInputElement>("New password").value).toBe("battery staple");
  expect(screen.getAllByRole("alert").filter((alert) => alert.textContent !== "")).toStrictEqual(
    [],
  );
  await expect(screen.findByRole("button", { name: "Change password" })).resolves.toBeDefined();
  expect(changedTo).toStrictEqual([]);
});

test("a wrong password in “Confirm it’s you” says so, and you can try again", async () => {
  const changedTo = openSettingsNeedingRecentAuthentication();

  await submitPasswordChange();
  const dialog = await confirmItsYou("not my password");
  await expect(within(dialog).findByText("That isn’t your password.")).resolves.toBeDefined();
  await confirmItsYou();

  await expect(screen.findByText("Password changed")).resolves.toBeDefined();
  expect(changedTo).toStrictEqual(["battery staple"]);
});

test("refused again after you confirm it’s you, the change shows an error, not the dialog", async () => {
  openSettingsNeedingRecentAuthentication(true);

  await submitPasswordChange();
  await confirmItsYou();

  await expect(screen.findByText("Sign in again to do this.")).resolves.toBeDefined();
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

/**
 * Settings' Users section, with the Admin Sam and Alex, who isn't one, on a Server that refuses
 * `ResetPassword` and `SetPermissions` for want of Recent Authentication until `Reauthenticate`, or
 * always if `alwaysRefuse`. Returns the resets it made and the Permissions it gave the Users it
 * made Admin.
 */
function openUsersNeedingRecentAuthentication(alwaysRefuse = false) {
  let reauthenticated = false;
  const resets: [bigint, string][] = [];
  const promoted: [bigint, Permission[]][] = [];
  openApp(
    "/code?settings=users",
    fakeServer(
      { signedIn: admin },
      {
        reauthenticate: () => {
          reauthenticated = true;
          return {};
        },
      },
      {
        listUsers: () => ({
          users: [
            { id: 1n, name: "Sam", permissions: admin },
            { id: 2n, name: "Alex", permissions: newUser },
          ],
        }),
        resetPassword: ({ userId, newPassword }) => {
          if (alwaysRefuse || !reauthenticated) {
            throw recentAuthenticationRequired();
          }
          resets.push([userId, newPassword]);
          return {};
        },
        setPermissions: ({ userId, permissions }) => {
          if (alwaysRefuse || !reauthenticated) {
            throw recentAuthenticationRequired();
          }
          if (permissions.includes(Permission.ADMIN)) {
            promoted.push([userId, permissions]);
          }
          return {};
        },
      },
    ),
  );
  return { resets, promoted };
}

test("without Recent Authentication, an Admin resets a password once they confirm it’s them", async () => {
  const { resets } = openUsersNeedingRecentAuthentication();

  fireEvent.click(await screen.findByRole("link", { name: "Alex" }));
  fireEvent.click(await screen.findByRole("button", { name: "Reset password for Alex" }));
  const reset = await screen.findByRole("alertdialog", { name: "Reset Alex’s password?" });
  fireEvent.change(within(reset).getByLabelText("New password"), {
    target: { value: "battery staple" },
  });
  fireEvent.click(within(reset).getByRole("button", { name: "Reset password" }));
  await confirmItsYou();

  await waitFor(() => {
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
  expect(resets).toStrictEqual([[2n, "battery staple"]]);
});

test("changes refused together wait for one “Confirm it’s you”, then all go through", async () => {
  const { resets, promoted } = openUsersNeedingRecentAuthentication();

  fireEvent.click(await screen.findByRole("link", { name: "Alex" }));
  const alex = await screen.findByRole("region", { name: "Alex" });
  const promote = within(alex).getByRole("checkbox", { name: "Admin" });
  fireEvent.click(screen.getByRole("button", { name: "Reset password for Alex" }));
  const reset = await screen.findByRole("alertdialog", { name: "Reset Alex’s password?" });
  fireEvent.change(within(reset).getByLabelText("New password"), {
    target: { value: "battery staple" },
  });
  // Both before either is refused.
  fireEvent.click(promote);
  fireEvent.click(within(reset).getByRole("button", { name: "Reset password" }));
  await confirmItsYou();

  await waitFor(() => {
    expect(promoted).toStrictEqual([[2n, [...newUser, Permission.ADMIN]]]);
  });
  expect(resets).toStrictEqual([[2n, "battery staple"]]);
});

test("on a Server Linked to the Hub, “Confirm it’s you” offers your Account, which comes back here", async () => {
  // Going to another site would end the test's page, so where it went is only noted.
  const went: string[] = [];
  vi.spyOn(globalThis.location, "assign").mockImplementation((url) => {
    went.push(String(url));
  });
  const returnsTo: URL[] = [];
  openApp(
    "/code?settings=signin",
    fakeServer(
      {
        signedIn: newUser,
        serverService: { getServerInfo: () => ({ name: "Living Room", accountSignIn: true }) },
      },
      {
        // The pane's own, with the current password, which isn't enough here.
        reauthenticate: () => ({}),
        startAccountSignIn: ({ returnUrl }) => {
          returnsTo.push(new URL(returnUrl));
          return { url: "https://hub.example/server-sign-in/1", handle: "secret" };
        },
      },
      {
        changePassword: () => {
          throw recentAuthenticationRequired();
        },
      },
    ),
  );

  await submitPasswordChange();
  const dialog = await screen.findByRole("alertdialog", { name: "Confirm it’s you" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Sign in with your Account" }));

  await waitFor(() => {
    expect(went).toStrictEqual(["https://hub.example/server-sign-in/1"]);
  });
  expect(returnsTo.map((url) => [url.pathname, url.searchParams.get("redirect")])).toStrictEqual([
    ["/reauthenticate/account", "/code?settings=signin"],
  ]);
  expect(sessionStorage.getItem("nama.account-sign-in")).toBe("secret");
});

/** Opens Alex's page and ticks Admin there, returning its checkbox. */
async function tickAdminForAlex() {
  fireEvent.click(await screen.findByRole("link", { name: "Alex" }));
  const alex = await screen.findByRole("region", { name: "Alex" });
  const promote = within(alex).getByRole("checkbox", { name: "Admin" });
  fireEvent.click(promote);
  return promote;
}

test("a Permission ticked without Recent Authentication is saved once you confirm it’s you", async () => {
  const { promoted } = openUsersNeedingRecentAuthentication();

  await tickAdminForAlex();
  await confirmItsYou();

  await waitFor(() => {
    expect(promoted).toStrictEqual([[2n, [...newUser, Permission.ADMIN]]]);
  });
});

test("cancelling “Confirm it’s you” unticks the Permission it was for", async () => {
  const { promoted } = openUsersNeedingRecentAuthentication();

  const promote = await tickAdminForAlex();
  const dialog = await screen.findByRole("alertdialog", { name: "Confirm it’s you" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

  await waitFor(() => {
    expect(promote).toHaveProperty("checked", false);
  });
  expect(promoted).toStrictEqual([]);
});

test("a Permission refused again after you confirm it’s you is unticked, and says so", async () => {
  openUsersNeedingRecentAuthentication(true);

  const promote = await tickAdminForAlex();
  await confirmItsYou();

  await expect(screen.findByText("Sign in again to do this.")).resolves.toBeDefined();
  expect(promote).toHaveProperty("checked", false);
});
