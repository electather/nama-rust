import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { DeviceSchema, Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, fill, newUser, openApp } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";

afterEach(cleanup);

/** Settings' Users section, the address a test opens it at. */
const usersSection = "/code?settings=users";

test("settings opens on Users for a User who manages Users", async () => {
  openApp(
    usersSection,
    fakeServer({ signedIn: [Permission.MANAGE_USERS] }, {}, { listUsers: () => ({}) }),
  );

  await expect(screen.findByRole("list", { name: "Users" })).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Users" }).getAttribute("aria-current")).toBe("page");
});

test("users isn't in Settings for a User who doesn't manage Users", async () => {
  openApp(usersSection, fakeServer({ signedIn: newUser }));

  await screen.findByRole("link", { name: "Playback", current: "page" });
  expect(screen.queryByRole("link", { name: "Users" })).toBeNull();
});

test("a User opens in place of the list, and the crumb over it goes back", async () => {
  const router = openApp(
    usersSection,
    fakeServer(
      { signedIn: admin },
      {},
      { listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }) },
    ),
  );

  fireEvent.click(await screen.findByRole("link", { name: "Alex" }));
  const opened = await screen.findByRole("region", { name: "Alex" });
  expect(router.state.location.search).toMatchObject({ settings: "users", user: 2 });
  expect(router.state.location.searchStr).toBe("?settings=users&user=2");
  const crumbs = within(opened).getByRole("navigation", { name: "Breadcrumb" });
  expect(within(crumbs).getByText("Alex").getAttribute("aria-current")).toBe("page");
  expect(
    within(crumbs).getByRole("link", { name: "Users" }).getAttribute("aria-current"),
  ).toBeNull();
  expect(within(opened).getByRole("button", { name: "Delete Alex" })).toBeDefined();
  expect(screen.queryByRole("list", { name: "Users" })).toBeNull();

  fireEvent.click(within(crumbs).getByRole("link", { name: "Users" }));
  await expect(screen.findByRole("list", { name: "Users" })).resolves.toBeDefined();
  expect(router.state.location.search.user).toBeUndefined();
});

test("a User deleted meanwhile is said to be gone, and Settings stays", async () => {
  const sam = { id: 1n, name: "Sam", permissions: admin };
  let users = [sam, { id: 2n, name: "Alex", permissions: newUser }];
  const devices = [create(DeviceSchema, { id: 3n, name: "Phone", createdAt: 1_700_000_000n })];
  let listDevices = () => ({ devices });
  openApp(
    usersSection,
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

  fireEvent.click(await screen.findByRole("link", { name: "Alex" }));
  fireEvent.click(await screen.findByRole("button", { name: "Revoke Phone" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Revoke" }),
  );

  await expect(screen.findByText("Alex isn’t on this Server any more.")).resolves.toBeDefined();
  expect(screen.getByRole("heading", { name: "Settings" })).toBeDefined();
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
      usersSection,
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
