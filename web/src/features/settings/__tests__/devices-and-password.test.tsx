import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { DeviceSchema } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { refused } from "@/test-refusal";

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

  await expect(
    screen.findByText("Password changed. Your other Devices were signed out."),
  ).resolves.toBeDefined();
  await waitFor(() => {
    expect(screen.queryByText("Living room TV")).toBeNull();
  });
});

test("a User who isn't an Admin sees no Admin entry in the chrome", async () => {
  openApp("/settings", fakeServer({ signedIn: newUser }));

  await screen.findByRole("link", { name: "Home" });
  expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
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
