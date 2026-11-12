import { Code } from "@connectrpc/connect";
import { fireEvent, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { UpdateImageSettingsRequest } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, fill, newUser, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

const manageSettings = [...newUser, Permission.MANAGE_SETTINGS];

test("a User who manages settings turns resizing off and limits its cache on Settings", async () => {
  const updates: UpdateImageSettingsRequest[] = [];
  openApp(
    "/settings",
    fakeServer({
      signedIn: manageSettings,
      serverService: {
        getImageSettings: () => ({ settings: { resizeImages: true } }),
        updateImageSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    }),
  );

  const resize = await screen.findByLabelText("Resize images");
  expect(resize).toHaveProperty("checked", true);
  fireEvent.click(resize);
  fill("Space for resized images", "512");
  fireEvent.click(screen.getByRole("button", { name: "Save image settings" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(updates).toMatchObject([{ settings: { resizeImages: false, resizedCacheMib: 512 } }]);
});

test("no space for resized images is refused under its field", async () => {
  openApp(
    "/settings",
    fakeServer({
      signedIn: manageSettings,
      serverService: {
        updateImageSettings: () => {
          throw refused(Code.InvalidArgument, "the resized copies must be allowed at least 1 MiB", {
            reason: RefusalReason.FIELD_REQUIRED,
            field: "settings.resized_cache_mib",
          });
        },
      },
    }),
  );

  await screen.findByLabelText("Space for resized images");
  fill("Space for resized images", "2");
  fireEvent.click(screen.getByRole("button", { name: "Save image settings" }));

  await expectUnderField(
    "Space for resized images",
    "The resized copies must be allowed at least 1 MiB.",
  );
});

test("a User who doesn't manage settings sees no image settings on Settings", async () => {
  openApp("/settings", fakeServer({ signedIn: newUser }));

  await screen.findByRole("heading", { name: "Devices" });

  expect(screen.queryByLabelText("Resize images")).toBeNull();
});
