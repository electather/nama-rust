import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, fill, newUser, openApp } from "./test-app";
import { confirmItsYou, recentAuthenticationRequired, refused } from "./test-refusal";

afterEach(cleanup);

async function submitPasswordChange() {
  await screen.findByRole("button", { name: "Change password" });
  fill("New password", "battery staple");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));
}

/**
 * Settings, on a Server that refuses `ChangePassword` for want of Recent Authentication until
 * `Reauthenticate` gets "correct horse", or always if `alwaysRefuse`. Returns the passwords it
 * changed to.
 */
function openSettingsNeedingRecentAuthentication(alwaysRefuse = false) {
  let reauthenticated = false;
  const changedTo: string[] = [];
  openApp(
    "/settings",
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
          reauthenticated = true;
          return {};
        },
      },
      {
        changePassword: ({ newPassword }) => {
          if (alwaysRefuse || !reauthenticated) throw recentAuthenticationRequired();
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

  expect(
    await screen.findByText("Password changed. Your other Devices were signed out."),
  ).toBeDefined();
  expect(changedTo).toEqual(["battery staple"]);
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
  expect(screen.getAllByRole("alert").map((alert) => alert.textContent)).toEqual(["", ""]);
  expect(await screen.findByRole("button", { name: "Change password" })).toBeDefined();
  expect(changedTo).toEqual([]);
});

test("a wrong password in “Confirm it’s you” says so, and you can try again", async () => {
  const changedTo = openSettingsNeedingRecentAuthentication();

  await submitPasswordChange();
  const dialog = await confirmItsYou("not my password");
  expect(await within(dialog).findByText("That isn’t your password.")).toBeDefined();
  await confirmItsYou();

  expect(
    await screen.findByText("Password changed. Your other Devices were signed out."),
  ).toBeDefined();
  expect(changedTo).toEqual(["battery staple"]);
});

test("refused again after you confirm it’s you, the change shows an error, not the dialog", async () => {
  openSettingsNeedingRecentAuthentication(true);

  await submitPasswordChange();
  await confirmItsYou();

  expect(await screen.findByText("Sign in again to do this.")).toBeDefined();
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

/**
 * The Users page, with the Admin Sam and Alex and Kim, who aren't Admins, on a Server that refuses
 * `ResetPassword` and `SetPermissions` for want of Recent Authentication until `Reauthenticate`. Returns
 * the resets it made and the Users it made Admin.
 */
function openUsersNeedingRecentAuthentication() {
  let reauthenticated = false;
  const resets: [bigint, string][] = [];
  const promoted: bigint[] = [];
  openApp(
    "/admin/users",
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
            { id: 3n, name: "Kim", permissions: newUser },
          ],
        }),
        resetPassword: ({ userId, newPassword }) => {
          if (!reauthenticated) throw recentAuthenticationRequired();
          resets.push([userId, newPassword]);
          return {};
        },
        setPermissions: ({ userId, permissions }) => {
          if (!reauthenticated) throw recentAuthenticationRequired();
          if (permissions.includes(Permission.ADMIN)) promoted.push(userId);
          return {};
        },
      },
    ),
  );
  return { resets, promoted };
}

test("without Recent Authentication, an Admin resets a password once they confirm it’s them", async () => {
  const { resets } = openUsersNeedingRecentAuthentication();

  fireEvent.click(await screen.findByText("Alex"));
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
  expect(resets).toEqual([[2n, "battery staple"]]);
});

test("changes refused together wait for one “Confirm it’s you”, then all go through", async () => {
  const { promoted } = openUsersNeedingRecentAuthentication();

  fireEvent.click(await screen.findByText("Alex"));
  fireEvent.click(await screen.findByText("Kim"));
  const makeAdmin = await screen.findAllByRole("button", { name: "Make Admin" });
  for (const button of makeAdmin) fireEvent.click(button);
  await confirmItsYou();

  await waitFor(() => {
    expect(promoted).toHaveLength(2);
  });
  expect(promoted).toEqual(expect.arrayContaining([2n, 3n]));
});
