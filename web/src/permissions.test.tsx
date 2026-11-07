import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, newUser, openApp, type ServerState } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

test("saved Permissions are there after reloading", async () => {
  let alex = { id: 2n, name: "Alex", permissions: [...newUser] as Permission[] };
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
  openApp("/admin/users", server);

  fireEvent.click(await screen.findByText("Alex"));
  fireEvent.click(await screen.findByRole("checkbox", { name: "Manage Libraries" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "View Requests" }));
  fireEvent.click(screen.getByRole("button", { name: "Save Permissions" }));
  await screen.findByText("Permissions saved.");
  cleanup();
  openApp("/admin/users", server);
  fireEvent.click(await screen.findByText("Alex"));

  expect(await screen.findByRole("checkbox", { name: "Manage Libraries" })).toHaveProperty(
    "checked",
    true,
  );
  expect(screen.getByRole("checkbox", { name: "View Requests" })).toHaveProperty("checked", false);
  expect(alex.permissions).toEqual([Permission.REQUEST, Permission.MANAGE_LIBRARIES]);
});

test("a checked parent Permission shows its children as covered", async () => {
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: admin },
      {},
      { listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }) },
    ),
  );

  fireEvent.click(await screen.findByText("Alex"));
  const movies = await screen.findByRole("checkbox", { name: "Request Movies" });
  expect(movies).toMatchObject({ checked: true, disabled: true });

  fireEvent.click(screen.getByRole("checkbox", { name: "Request" }));
  expect(movies).toMatchObject({ checked: false, disabled: false });
});

test.each([
  [
    "removing the last Admin",
    refused(Code.FailedPrecondition, "refused", { reason: RefusalReason.LAST_ADMIN }),
    "Sam is the only Admin. Make someone else an Admin first.",
  ],
  [
    "a change only an Admin may make",
    refused(Code.PermissionDenied, "refused", {
      reason: RefusalReason.PERMISSION_REQUIRED,
      params: { permission: "PERMISSION_ADMIN" },
    }),
    "This needs the Admin Permission.",
  ],
])("saving Permissions says why the Server refused %s", async (_, refusal, shown) => {
  openApp(
    "/admin/users",
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

  fireEvent.click(await screen.findByText("Sam"));
  fireEvent.click(await screen.findByRole("checkbox", { name: "Admin" }));
  fireEvent.click(screen.getByRole("button", { name: "Save Permissions" }));

  expect(await screen.findByText(shown)).toBeDefined();
});

test("removing your own Admin Permission leaves the Admin pages", async () => {
  const state: ServerState = { signedIn: admin };
  const users = [
    { id: 1n, name: "Sam", permissions: admin },
    { id: 2n, name: "Alex", permissions: admin },
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

  fireEvent.click(await screen.findByText("Sam"));
  fireEvent.click(await screen.findByRole("checkbox", { name: "Admin" }));
  fireEvent.click(screen.getByRole("button", { name: "Save Permissions" }));

  expect(await screen.findByRole("heading", { name: "You can’t open this page" })).toBeDefined();
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
});

test("removing your own Admin Permission but keeping Manage Users leaves only Users in the Admin nav", async () => {
  const state: ServerState = { signedIn: admin };
  let sam = { id: 1n, name: "Sam", permissions: [...admin] as Permission[] };
  openApp(
    "/admin/users",
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

  fireEvent.click(await screen.findByText("Sam"));
  fireEvent.click(await screen.findByRole("checkbox", { name: "Admin" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Manage Users" }));
  fireEvent.click(screen.getByRole("button", { name: "Save Permissions" }));

  const nav = await screen.findByRole("navigation", { name: "Admin" });
  await waitFor(() => {
    expect(within(nav).queryByRole("link", { name: "Libraries" })).toBeNull();
  });
  expect(within(nav).getByRole("link", { name: "Users" })).toBeDefined();
});

test("without the Admin Permission, only a non-Admin's Permissions can be changed, and never to Admin", async () => {
  openApp(
    "/admin/users",
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

  fireEvent.click(await screen.findByText("Sam"));
  fireEvent.click(screen.getByText("Alex"));
  fireEvent.click(screen.getByText("Pat"));

  expect(await screen.findAllByRole("button", { name: "Save Permissions" })).toHaveLength(1);
  expect(screen.getByRole("checkbox", { name: "Request" })).toBeDefined();
  expect(screen.queryByRole("checkbox", { name: "Admin" })).toBeNull();
});

test("saved default Permissions are named on the Users page after reloading", async () => {
  let defaults = [...newUser] as Permission[];
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
  openApp("/admin/users", server);

  expect(await screen.findByText("Request · View Requests")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Change" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Request" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Request Movies" }));
  fireEvent.click(screen.getByRole("button", { name: "Save default Permissions" }));
  await screen.findByText("Default Permissions saved.");
  cleanup();
  openApp("/admin/users", server);

  expect(await screen.findByText("Request Movies · View Requests")).toBeDefined();
  expect(defaults).toEqual([Permission.VIEW_REQUESTS, Permission.REQUEST_MOVIE]);
});

test("without the Admin Permission, default Permissions holding Admin can't be changed", async () => {
  openApp(
    "/admin/users",
    fakeServer(
      { signedIn: [Permission.MANAGE_USERS] },
      {},
      {
        listUsers: () => ({ users: [] }),
        getDefaultPermissions: () => ({ permissions: admin }),
      },
    ),
  );

  const defaults = await screen.findByRole("region", { name: "Default Permissions" });
  expect(within(defaults).getByText("Admin")).toBeDefined();
  expect(within(defaults).queryByRole("button", { name: "Change" })).toBeNull();
});
