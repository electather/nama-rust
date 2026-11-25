import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { fakeServer, fill, newUser } from "@/test-app";
import {
  confirmItsYou,
  expectUnderField,
  recentAuthenticationRequired,
  refused,
} from "@/test-refusal";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

/**
 * The Sign-in pane, on a Server that asks to confirm it's you once before each change, and
 * records the Switch PINs set and the Users whose Switch PIN was cleared.
 */
function openSwitchPin() {
  const set: string[] = [];
  const cleared: (bigint | undefined)[] = [];
  const calls = { set, cleared, confirmed: false };
  const confirmFirst = () => {
    if (!calls.confirmed) {
      throw recentAuthenticationRequired();
    }
  };
  openSettings(
    "signin",
    fakeServer(
      { signedIn: newUser },
      {
        reauthenticate: () => {
          calls.confirmed = true;
          return {};
        },
        setSwitchPin: ({ switchPin }) => {
          confirmFirst();
          if (!/^\d{4,6}$/u.test(switchPin)) {
            throw refused(Code.InvalidArgument, "a Switch PIN must be 4 to 6 digits", {
              reason: RefusalReason.SWITCH_PIN_INVALID,
              field: "switch_pin",
            });
          }
          calls.set.push(switchPin);
          return {};
        },
        clearSwitchPin: ({ userId }) => {
          confirmFirst();
          calls.cleared.push(userId);
          return {};
        },
      },
    ),
  );
  return calls;
}

async function setPin(pin: string) {
  await screen.findByRole("button", { name: "Set Switch PIN" });
  fill("New Switch PIN", pin);
  fireEvent.click(screen.getByRole("button", { name: "Set Switch PIN" }));
}

test("setting a Switch PIN, once you confirm it's you, saves it", async () => {
  const calls = openSwitchPin();

  await setPin("2468");
  await confirmItsYou();

  await screen.findByText("Switch PIN set");
  expect(calls.set).toStrictEqual(["2468"]);
  expect(screen.getByLabelText<HTMLInputElement>("New Switch PIN").value).toBe("");
});

test("a Switch PIN the Server refuses is said under its field", async () => {
  const calls = openSwitchPin();
  calls.confirmed = true;

  await setPin("12");

  await expectUnderField("New Switch PIN", "A Switch PIN must be 4 to 6 digits.");
  expect(calls.set).toStrictEqual([]);
});

test("clearing your Switch PIN, once you confirm it's you, clears your own", async () => {
  const calls = openSwitchPin();

  fireEvent.click(await screen.findByRole("button", { name: "Clear Switch PIN" }));
  await confirmItsYou();

  await screen.findByText("Switch PIN cleared");
  expect(calls.cleared).toStrictEqual([undefined]);
});
