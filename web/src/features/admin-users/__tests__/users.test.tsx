import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { DeviceSchema } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, fill, newUser, openApp } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";

afterEach(cleanup);

test("a User deleted meanwhile is said to be gone, and the Users page stays", async () => {
  const sam = { id: 1n, name: "Sam", permissions: admin };
  let users = [sam, { id: 2n, name: "Alex", permissions: newUser }];
  const devices = [create(DeviceSchema, { id: 3n, name: "Phone", createdAt: 1_700_000_000n })];
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

  await expect(screen.findByText("Alex isn’t on this Server any more.")).resolves.toBeDefined();
  expect(screen.getByRole("heading", { name: "Users" })).toBeDefined();
});

test.each([
  [
    "name",
    {
      label: "Username",
      message: "a User's name needs 1 to 64 characters",
      shown: "A User's name needs 1 to 64 characters.",
    },
  ],
  [
    "password",
    {
      label: "Starting password",
      message: "the password needs at least 8 characters",
      shown: "The password needs at least 8 characters.",
    },
  ],
])(
  "a refused %s for a new User is shown under its field",
  async (field, { label, message, shown }) => {
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
  },
);
