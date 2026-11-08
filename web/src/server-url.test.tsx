import { Code } from "@connectrpc/connect";
import { fireEvent, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { UpdateServerSettingsRequest } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, fill, newUser, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

const manageSettings = [...newUser, Permission.MANAGE_SETTINGS];

test("a User who manages settings changes the Server URL on Settings", async () => {
  const updates: UpdateServerSettingsRequest[] = [];
  openApp(
    "/settings",
    fakeServer({
      signedIn: manageSettings,
      serverSettings: {
        getServerSettings: () => ({ settings: { url: "http://nama.lan" } }),
        updateServerSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );

  expect(await screen.findByLabelText("Server URL")).toHaveProperty("value", "http://nama.lan");
  fill("Server URL", "https://nama.example.com");
  fireEvent.click(screen.getByRole("button", { name: "Save Server URL" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([{ settings: { url: "https://nama.example.com" } }]);
});

test("a Server URL that isn't a URL is refused under its field", async () => {
  openApp(
    "/settings",
    fakeServer({
      signedIn: manageSettings,
      serverSettings: {
        updateServerSettings: () => {
          throw refused(Code.InvalidArgument, "the Server URL must be an http or https URL", {
            reason: RefusalReason.NOT_A_URL,
            field: "settings.url",
          });
        },
      },
    }),
  );

  await screen.findByLabelText("Server URL");
  fill("Server URL", "ftp://nama.example.com");
  fireEvent.click(screen.getByRole("button", { name: "Save Server URL" }));

  await expectUnderField("Server URL", "The Server URL must be an http or https URL.");
});

test("a User who doesn't manage settings sees no Server URL on Settings", async () => {
  openApp("/settings", fakeServer({ signedIn: newUser }));

  await screen.findByRole("heading", { name: "Devices" });

  expect(screen.queryByLabelText("Server URL")).toBeNull();
});
