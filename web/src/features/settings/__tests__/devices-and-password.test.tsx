import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { DeviceSchema } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, fill, newUser } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

function device(id: bigint, name: string, isCurrent = false) {
  return create(DeviceSchema, { id, name, createdAt: 1_700_000_000n, isCurrent });
}

test("revoking a Device, once confirmed, removes it from Settings", async () => {
  let devices = [device(1n, "Laptop", true), device(2n, "Living room TV")];
  openSettings(
    "devices",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        listDevices: () => ({ devices }),
        revokeDevice: ({ deviceId }) => {
          devices = devices.filter((each) => each.id !== deviceId);
          return {};
        },
      },
    ),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Revoke Living room TV" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Revoke Living room TV?" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Revoke Device" }));

  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
  expect(screen.getByText("Laptop")).toBeDefined();
  expect(screen.getByText("This Device")).toBeDefined();
});

test("a Device revoked elsewhere meanwhile just leaves the list", async () => {
  let devices = [device(1n, "Laptop", true), device(2n, "Living room TV")];
  openSettings(
    "devices",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        listDevices: () => ({ devices }),
        revokeDevice: () => {
          devices = devices.slice(0, 1);
          throw refused(Code.NotFound, "no such Device", {
            reason: RefusalReason.DEVICE_NOT_FOUND,
          });
        },
      },
    ),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Revoke Living room TV" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Revoke Device" }),
  );

  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("the Devices pane signs in another Device in place", async () => {
  openSettings("devices", fakeServer({ signedIn: newUser }));

  fireEvent.click(await screen.findByRole("button", { name: "Sign in a Device" }));
  const settings = screen.getByRole("dialog", { name: "Settings" });
  expect(within(settings).getByRole("textbox", { name: "Sign-in Code" })).toBeDefined();
});

test("pressing Escape in the Sign-in Code cancels signing in a Device, and Settings stays open", async () => {
  openSettings("devices", fakeServer({ signedIn: newUser }));

  fireEvent.click(await screen.findByRole("button", { name: "Sign in a Device" }));
  const settings = within(screen.getByRole("dialog", { name: "Settings" }));
  const code = settings.getByRole("textbox", { name: "Sign-in Code" });

  // The dialog closes on an Escape keydown whose default nobody prevented.
  expect(fireEvent.keyDown(code, { key: "Escape" })).toBe(false);
  expect(settings.queryByRole("textbox", { name: "Sign-in Code" })).toBeNull();
  expect(document.activeElement).toBe(settings.getByRole("button", { name: "Sign in a Device" }));
});

/** The Sign-in pane, on a Server whose password is "correct horse", which records what it changed to. */
function openSignin(answer: () => object = () => ({})) {
  const changedTo: string[] = [];
  openSettings(
    "signin",
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
          return {};
        },
      },
      {
        changePassword: ({ newPassword }) => {
          changedTo.push(newPassword);
          return answer();
        },
      },
    ),
  );
  return changedTo;
}

async function changePassword(current: string, next: string, again = next) {
  await screen.findByRole("button", { name: "Change password" });
  fill("Current password", current);
  fill("New password", next);
  fill("Confirm new password", again);
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));
}

test("your username shows, and changing your password with your current one signs out your other Devices", async () => {
  const changedTo = openSignin();

  await expect(screen.findByText("Sam")).resolves.toBeDefined();
  expect(screen.getByText("Your other Devices will be signed out.")).toBeDefined();
  await changePassword("correct horse", "battery staple");

  await expect(screen.findByText("Password changed")).resolves.toBeDefined();
  expect(screen.getByText("Your other Devices are signed out.")).toBeDefined();
  expect(changedTo).toStrictEqual(["battery staple"]);
  // The current password confirmed it's you, so nothing asks again.
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(screen.getByLabelText<HTMLInputElement>("New password").value).toBe("");
});

test("a wrong current password is said under its field, and nothing changes", async () => {
  const changedTo = openSignin();

  await changePassword("correct hose", "battery staple");

  await expectUnderField("Current password", "That isn’t your current password.");
  expect(changedTo).toStrictEqual([]);
});

test("what's missing is said under each field before the Server is asked", async () => {
  const changedTo = openSignin();

  await changePassword("", "horse", "hose");

  await expectUnderField("Current password", "Enter your current password.");
  await expectUnderField("New password", "Use at least 8 characters.");
  await expectUnderField("Confirm new password", "This doesn’t match the new password.");
  expect(changedTo).toStrictEqual([]);
});

test("a Device signed out meanwhile is told so", async () => {
  openSignin(() => {
    throw refused(Code.Unauthenticated, "this Device was signed out", {
      reason: RefusalReason.SIGNED_OUT,
    });
  });

  await changePassword("correct horse", "battery staple");

  await screen.findByText("This Device was signed out.");
  expect(screen.getByLabelText("New password").getAttribute("aria-invalid")).toBe("false");
});
