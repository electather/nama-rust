import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { DeviceSchema, Role } from "@/gen/nama/v1/sign_in_pb";

import { type ServerState, fakeServer, fill, openApp } from "./test-app";
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
      { signedIn: Role.MEMBER },
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
      { signedIn: Role.MEMBER },
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
      { signedIn: Role.MEMBER },
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
  fill("Current password", "correct horse");
  fill("New password", "battery staple");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));

  expect(
    await screen.findByText("Password changed. Your other Devices were signed out."),
  ).toBeDefined();
  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
});

test("a Member sees no Admin entry in the chrome", async () => {
  openApp("/settings", fakeServer({ signedIn: Role.MEMBER }));

  await screen.findByRole("link", { name: "Settings" });
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("demoting the last Admin says why it was refused", async () => {
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: Role.ADMIN },
      {},
      {
        listUsers: () => ({ users: [{ id: 1n, name: "Sam", role: Role.ADMIN }] }),
        setRole: () => {
          throw refused(Code.FailedPrecondition, "the Server must keep at least one Admin", {
            reason: RefusalReason.LAST_ADMIN,
          });
        },
      },
    ),
  );

  fireEvent.click(await screen.findByText("Sam"));
  fireEvent.click(await screen.findByRole("button", { name: "Make Member" }));

  expect(
    await screen.findByText("Sam is the only Admin. Make someone else an Admin first."),
  ).toBeDefined();
});

test("demoting yourself leaves the Admin pages", async () => {
  const state: ServerState = { signedIn: Role.ADMIN };
  const users = [
    { id: 1n, name: "Sam", role: Role.ADMIN },
    { id: 2n, name: "Alex", role: Role.ADMIN },
  ];
  let listUsers = () => ({ users });
  openApp(
    "/admin/users",
    fakeServer(
      state,
      {},
      {
        listUsers: () => listUsers(),
        // Sam demotes themselves, so the Server refuses them the Users from then on.
        setRole: ({ role }) => {
          state.signedIn = role;
          listUsers = () => {
            throw refused(Code.PermissionDenied, "Admins only", {
              reason: RefusalReason.ADMIN_REQUIRED,
            });
          };
          return {};
        },
      },
    ),
  );

  fireEvent.click(await screen.findByText("Sam"));
  fireEvent.click(await screen.findByRole("button", { name: "Make Member" }));

  expect(await screen.findByRole("heading", { name: "Admins only" })).toBeDefined();
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("a User deleted meanwhile is said to be gone, and the Users page stays", async () => {
  const sam = { id: 1n, name: "Sam", role: Role.ADMIN };
  let users = [sam, { id: 2n, name: "Alex", role: Role.MEMBER }];
  const devices = [device(3n, "Phone")];
  let listDevices = () => ({ devices });
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: Role.ADMIN },
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
      { signedIn: Role.ADMIN },
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

/** Settings, on a Server whose `ChangePassword` refuses with `reason` and `message`, about `field`. */
function openSettingsRefusingPasswordChange(reason: RefusalReason, message: string, field = "") {
  openApp(
    "/settings",
    fakeServer(
      { signedIn: Role.MEMBER },
      {},
      {
        changePassword: () => {
          throw refused(Code.Unauthenticated, message, { reason, field });
        },
      },
    ),
  );
}

async function submitPasswordChange() {
  await screen.findByRole("button", { name: "Change password" });
  fill("Current password", "correct horse");
  fill("New password", "battery staple");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));
}

test("a wrong current password is shown under its field", async () => {
  openSettingsRefusingPasswordChange(
    RefusalReason.WRONG_PASSWORD,
    "the current password is wrong",
    "current_password",
  );

  await submitPasswordChange();
  await expectUnderField("Current password", "That isn’t your current password.");
});

test("a Device signed out meanwhile isn't blamed on the current password", async () => {
  openSettingsRefusingPasswordChange(RefusalReason.SIGNED_OUT, "this Device was signed out");

  await submitPasswordChange();
  await screen.findByText("This Device was signed out.");
  expect(screen.getByLabelText("Current password").getAttribute("aria-invalid")).toBe("false");
});
