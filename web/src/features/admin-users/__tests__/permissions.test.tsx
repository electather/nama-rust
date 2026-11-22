import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, newUser, openApp, type ServerState } from "@/test-app";
import { refused } from "@/test-refusal";

afterEach(cleanup);

/** Settings' Users section, the address a test opens it at. */
const usersSection = "/code?settings=users";

/** Opens `name`'s details from the list of Users, once they're in its place. */
async function openUser(name: string) {
  fireEvent.click(await screen.findByRole("link", { name }));
  await screen.findByRole("region", { name });
}

/** A User the fake Server keeps, whose Permissions a test changes. */
interface Holder {
  id: bigint;
  name: string;
  permissions: Permission[];
}

test("a User's Permissions are saved as they're changed, and are there after reloading", async () => {
  let alex: Holder = { id: 2n, name: "Alex", permissions: [...newUser] };
  const server = fakeServer(
    { signedIn: admin },
    {},
    {
      listUsers: () => ({ users: [alex] }),
      setPermissions: ({ permissions }) => {
        alex = { ...alex, permissions };
        return {};
      },
    },
  );
  openApp(usersSection, server);

  await openUser("Alex");
  fireEvent.click(await screen.findByRole("checkbox", { name: "Manage Libraries" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "View Requests" }));
  await waitFor(() => {
    expect(alex.permissions).toStrictEqual([Permission.REQUEST, Permission.MANAGE_LIBRARIES]);
  });
  cleanup();
  openApp(usersSection, server);
  await openUser("Alex");

  await expect(screen.findByRole("checkbox", { name: "Manage Libraries" })).resolves.toHaveProperty(
    "checked",
    true,
  );
  expect(screen.getByRole("checkbox", { name: "View Requests" })).toHaveProperty("checked", false);
  expect(screen.queryByRole("button", { name: /Save/u })).toBeNull();
});

test("a checked parent Permission shows its children as covered", async () => {
  openApp(
    usersSection,
    fakeServer(
      { signedIn: admin },
      {},
      {
        listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }),
        setPermissions: () => ({}),
      },
    ),
  );

  await openUser("Alex");
  const movies = await screen.findByRole("checkbox", { name: "Request Movies" });
  expect(movies).toMatchObject({ checked: true, disabled: true });

  fireEvent.click(screen.getByRole("checkbox", { name: "Request" }));
  expect(movies).toMatchObject({ checked: false, disabled: false });
});

test.each([
  [
    "removing the last Admin",
    refused(Code.FailedPrecondition, "refused", { reason: RefusalReason.LAST_ADMIN }),
    "Sam is the only Admin who can sign in without the Hub. Make another User with a password an Admin first.",
  ],
  [
    "a change only an Admin may make",
    refused(Code.PermissionDenied, "refused", {
      reason: RefusalReason.PERMISSION_REQUIRED,
      params: { permission: "PERMISSION_ADMIN" },
    }),
    "This needs the Admin Permission.",
  ],
])(
  "a change to Permissions the Server refuses for %s is undone, and says why",
  async (_, refusal, shown) => {
    openApp(
      usersSection,
      fakeServer(
        { signedIn: admin },
        {},
        {
          listUsers: () => ({ users: [{ id: 1n, name: "Sam", permissions: admin }] }),
          setPermissions: () => {
            throw refusal;
          },
        },
      ),
    );

    await openUser("Sam");
    const adminBox = await screen.findByRole("checkbox", { name: "Admin" });
    fireEvent.click(adminBox);

    await expect(screen.findByText(shown)).resolves.toBeDefined();
    expect(adminBox).toHaveProperty("checked", true);
  },
);

test("removing your own Admin Permission takes Users out of Settings", async () => {
  const state: ServerState = { signedIn: admin };
  const users = [
    { id: 1n, name: "Sam", permissions: admin },
    { id: 2n, name: "Alex", permissions: admin },
  ];
  let listUsers = () => ({ users });
  openApp(
    usersSection,
    fakeServer(
      state,
      {},
      {
        listUsers: () => listUsers(),
        // Sam demotes themselves, so the Server refuses them the Users from then on.
        setPermissions: ({ permissions }) => {
          state.signedIn = permissions;
          listUsers = () => {
            throw refused(Code.PermissionDenied, "this needs the MANAGE_USERS Permission", {
              reason: RefusalReason.PERMISSION_REQUIRED,
              params: { permission: "PERMISSION_MANAGE_USERS" },
            });
          };
          return {};
        },
      },
    ),
  );

  await openUser("Sam");
  fireEvent.click(await screen.findByRole("checkbox", { name: "Admin" }));

  await expect(
    screen.findByRole("link", { name: "Playback", current: "page" }),
  ).resolves.toBeDefined();
  expect(screen.queryByRole("link", { name: "Users" })).toBeNull();
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("removing your own Admin Permission but keeping Manage Users leaves only Users in Settings' Admin group, and the Admin link", async () => {
  const state: ServerState = { signedIn: admin };
  let sam: Holder = { id: 1n, name: "Sam", permissions: [...admin] };
  openApp(
    usersSection,
    fakeServer(
      state,
      {},
      {
        listUsers: () => ({ users: [sam] }),
        setPermissions: ({ permissions }) => {
          state.signedIn = permissions;
          sam = { ...sam, permissions };
          return {};
        },
      },
    ),
  );

  await openUser("Sam");
  fireEvent.click(await screen.findByRole("checkbox", { name: "Admin" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Manage Users" }));

  await waitFor(() => {
    expect(screen.queryByRole("link", { name: "Server" })).toBeNull();
  });
  expect(screen.getByRole("region", { name: "Sam" })).toBeDefined();
  expect(screen.getByRole("link", { name: "Admin" })).toBeDefined();
});

// Sam is you, and Pat is an Admin.
test.each([
  { name: "Sam", changeable: false },
  { name: "Alex", changeable: true },
  { name: "Pat", changeable: false },
])(
  "without the Admin Permission, whether $name's Permissions can be changed is $changeable, never to Admin",
  async ({ name, changeable }) => {
    openApp(
      usersSection,
      fakeServer(
        { signedIn: [Permission.MANAGE_USERS] },
        {},
        {
          listUsers: () => ({
            users: [
              { id: 1n, name: "Sam", permissions: [Permission.MANAGE_USERS] },
              { id: 2n, name: "Alex", permissions: newUser },
              { id: 3n, name: "Pat", permissions: admin },
            ],
          }),
        },
      ),
    );

    await openUser(name);

    await screen.findByRole("button", { name: /^Delete /u });
    expect(screen.queryByRole("checkbox", { name: "Request" }) !== null).toBe(changeable);
    expect(screen.queryByRole("checkbox", { name: "Admin" })).toBeNull();
  },
);

test("default Permissions are saved as they're changed, and are there after reloading", async () => {
  let defaults: Permission[] = [...newUser];
  const server = fakeServer(
    { signedIn: admin },
    {},
    {
      listUsers: () => ({ users: [] }),
      getDefaultPermissions: () => ({ permissions: defaults }),
      setDefaultPermissions: ({ permissions }) => {
        defaults = permissions;
        return {};
      },
    },
  );
  openApp(usersSection, server);

  fireEvent.click(await screen.findByRole("checkbox", { name: "Request" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Request Movies" }));
  await waitFor(() => {
    expect(defaults).toStrictEqual([Permission.VIEW_REQUESTS, Permission.REQUEST_MOVIE]);
  });
  cleanup();
  openApp(usersSection, server);

  await expect(screen.findByRole("checkbox", { name: "Request Movies" })).resolves.toHaveProperty(
    "checked",
    true,
  );
  expect(screen.getByRole("checkbox", { name: "Request" })).toHaveProperty("checked", false);
  expect(screen.queryByRole("button", { name: /Save/u })).toBeNull();
});

test("a refused change to the default Permissions is undone, and says why", async () => {
  openApp(
    usersSection,
    fakeServer(
      { signedIn: admin },
      {},
      {
        listUsers: () => ({ users: [] }),
        getDefaultPermissions: () => ({ permissions: newUser }),
        setDefaultPermissions: () => {
          throw refused(Code.PermissionDenied, "refused", {
            reason: RefusalReason.PERMISSION_REQUIRED,
            params: { permission: "PERMISSION_ADMIN" },
          });
        },
      },
    ),
  );

  const request = await screen.findByRole("checkbox", { name: "Request" });
  fireEvent.click(request);

  await expect(screen.findByText("This needs the Admin Permission.")).resolves.toBeDefined();
  expect(request).toHaveProperty("checked", true);
});

test("without the Admin Permission, default Permissions holding Admin are named, not changed", async () => {
  openApp(
    usersSection,
    fakeServer(
      { signedIn: [Permission.MANAGE_USERS] },
      {},
      {
        listUsers: () => ({ users: [] }),
        getDefaultPermissions: () => ({ permissions: [Permission.ADMIN, Permission.REQUEST] }),
      },
    ),
  );

  await expect(screen.findByText("Admin · Request")).resolves.toBeDefined();
  expect(screen.queryByRole("checkbox", { name: "Admin" })).toBeNull();
  expect(screen.queryByRole("checkbox", { name: "Request" })).toBeNull();
});
