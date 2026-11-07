import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
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
