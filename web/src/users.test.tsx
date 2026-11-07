import { create } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { DeviceSchema, Role } from "@/gen/nama/v1/sign_in_pb";

import { type ServerState, fakeServer, openApp } from "./test-app";

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
          throw new ConnectError(
            "the Server must keep at least one Admin",
            Code.FailedPrecondition,
          );
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
            throw new ConnectError("Admins only", Code.PermissionDenied);
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
