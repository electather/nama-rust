import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { NotificationKind, type SetNotificationPreferenceRequest } from "@/gen/nama/v1/user_pb";

import { fakeServer, newUser, openApp } from "./test-app";

test("a User turns off Approved notifications on Settings and keeps Available", async () => {
  // This User only ever turns kinds off.
  const off = new Set<NotificationKind>();
  const set: SetNotificationPreferenceRequest[] = [];
  const transport = fakeServer(
    { signedIn: newUser },
    {},
    {
      listNotificationPreferences: () => ({
        preferences: [NotificationKind.APPROVED, NotificationKind.AVAILABLE].map((kind) => ({
          kind,
          on: !off.has(kind),
        })),
      }),
      setNotificationPreference: (request) => {
        set.push(request);
        off.add(request.kind);
        return {};
      },
    },
  );
  openApp("/settings", transport);

  const notifications = await screen.findByRole("region", { name: "Notifications" });
  const approved = within(notifications).getByRole<HTMLInputElement>("checkbox", {
    name: "Your Request is approved",
  });
  fireEvent.click(approved);

  await expect.poll(() => set).toMatchObject([{ kind: NotificationKind.APPROVED, on: false }]);
  await expect.poll(() => approved.checked).toBe(false);
  expect(
    within(notifications).getByRole<HTMLInputElement>("checkbox", {
      name: "Your Request is Available",
    }).checked,
  ).toBe(true);
});
