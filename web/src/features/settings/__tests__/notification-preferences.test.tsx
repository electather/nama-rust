import { fireEvent, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { NotificationKind, type SetNotificationPreferenceRequest } from "@/gen/nama/v1/user_pb";
import { fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

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
  openSettings("connections", transport);

  const approved = await screen.findByRole("switch", { name: "Your Request is approved" });
  expect(approved.getAttribute("aria-checked")).toBe("true");
  fireEvent.click(approved);

  await expect.poll(() => set).toMatchObject([{ kind: NotificationKind.APPROVED, on: false }]);
  await expect.poll(() => approved.getAttribute("aria-checked")).toBe("false");
  expect(
    screen.getByRole("switch", { name: "Your Request is Available" }).getAttribute("aria-checked"),
  ).toBe("true");
});
