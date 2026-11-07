import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { DeviceSchema } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, fill, newUser, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

function device(id: bigint, name: string, isCurrent = false) {
  return create(DeviceSchema, { id, name, createdAt: 1_700_000_000n, isCurrent });
}

test("revoking a Device, once confirmed, removes it from Settings", async () => {
  let devices = [device(1n, "Laptop", true), device(2n, "Living room TV")];
  openApp(
    "/settings",
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
  const dialog = await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("button", { name: "Revoke" }));

  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
  expect(screen.getByText("Laptop")).toBeDefined();
});

test("a Device revoked elsewhere meanwhile just leaves the list", async () => {
  let devices = [device(1n, "Laptop", true), device(2n, "Living room TV")];
  openApp(
    "/settings",
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
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Revoke" }),
  );

  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

test("changing your password says your other Devices were signed out", async () => {
  let devices = [device(1n, "Laptop", true), device(2n, "Living room TV")];
  openApp(
    "/settings",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        listDevices: () => ({ devices }),
        changePassword: () => {
          devices = devices.slice(0, 1);
          return {};
        },
      },
    ),
  );

  await screen.findByText("Living room TV");
  fill("New password", "battery staple");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));

  expect(
    await screen.findByText("Password changed. Your other Devices were signed out."),
  ).toBeDefined();
  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
});

test("a User who isn't an Admin sees no Admin entry in the chrome", async () => {
  openApp("/settings", fakeServer({ signedIn: newUser }));

  await screen.findByRole("link", { name: "Settings" });
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("a User deleted meanwhile is said to be gone, and the Users page stays", async () => {
  const sam = { id: 1n, name: "Sam", permissions: admin };
  let users = [sam, { id: 2n, name: "Alex", permissions: newUser }];
  const devices = [device(3n, "Phone")];
  let listDevices = () => ({ devices });
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: admin },
      {},
      {
        listUsers: () => ({ users }),
        listDevices: () => listDevices(),
        // Another Admin deletes Alex while this one has Alex open.
        revokeDevice: () => {
          users = [sam];
          listDevices = () => {
            throw refused(Code.NotFound, "no such User", { reason: RefusalReason.USER_NOT_FOUND });
          };
          throw refused(Code.NotFound, "no such User", { reason: RefusalReason.USER_NOT_FOUND });
        },
      },
    ),
  );

  fireEvent.click(await screen.findByText("Alex"));
  fireEvent.click(await screen.findByRole("button", { name: "Revoke Phone" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Revoke" }),
  );

  expect(await screen.findByText("Alex isn’t on this Server any more.")).toBeDefined();
  expect(screen.getByRole("heading", { name: "Users" })).toBeDefined();
});

test.each([
  [
    "name",
    "Username",
    "a User's name needs 1 to 64 characters",
    "A User's name needs 1 to 64 characters.",
  ],
  [
    "password",
    "Starting password",
    "the password needs at least 8 characters",
    "The password needs at least 8 characters.",
  ],
])("a refused %s for a new User is shown under its field", async (field, label, message, shown) => {
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: admin },
      {},
      {
        listUsers: () => ({}),
        createUser: () => {
          throw refused(Code.InvalidArgument, message, { field });
        },
      },
    ),
  );

  await screen.findByRole("button", { name: "Create User" });
  fill("Username", "Alex");
  fill("Starting password", "short");
  fireEvent.click(screen.getByRole("button", { name: "Create User" }));

  await expectUnderField(label, shown);
});

/** Settings, on a Server whose `ChangePassword` refuses with `reason` and `message`. */
function openSettingsRefusingPasswordChange(reason: RefusalReason, message: string) {
  openApp(
    "/settings",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        changePassword: () => {
          throw refused(Code.Unauthenticated, message, { reason });
        },
      },
    ),
  );
}

async function submitPasswordChange() {
  await screen.findByRole("button", { name: "Change password" });
  fill("New password", "battery staple");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));
}

test("a Device signed out meanwhile is told so", async () => {
  openSettingsRefusingPasswordChange(RefusalReason.SIGNED_OUT, "this Device was signed out");

  await submitPasswordChange();
  await screen.findByText("This Device was signed out.");
  expect(screen.getByLabelText("New password").getAttribute("aria-invalid")).toBe("false");
});
