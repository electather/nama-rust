import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { serverSessionKey } from "@/features/servers/lib/server-sessions";
import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, fill, openApp } from "@/test-app";
import { accountHub, ada } from "@/test-hub";
import { recentAuthenticationRequired } from "@/test-refusal";
import { findSettingsLink, pick } from "@/test-settings";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const basement = create(LinkedServerSchema, {
  id: "basement",
  name: "Basement",
  addresses: ["https://basement.example"],
});

/**
 * Settings at the Hub, opened at `path`, for an Account that is an Admin on Basement, whose Hub
 * session is live if `signedIn`. Basement gives a Device Recent Authentication only from a trade
 * after the first, as one after a fresh Hub sign-in does. Returns the Switch PINs it set and how
 * many trades it took.
 */
function openBasement(path: string, signedIn = true) {
  const basementDid: { trades: number; pins: string[] } = { trades: 0, pins: [] };
  const fake = fakeServer(
    {
      signedIn: [Permission.ADMIN],
      serverService: {
        getServerInfo: () => ({ version: "0.3.0", settingsSections: ["signin"] }),
      },
    },
    {
      signInWithHubToken: () => {
        basementDid.trades += 1;
        return { token: `session ${basementDid.trades}` };
      },
      setSwitchPin: ({ switchPin }) => {
        if (basementDid.trades < 2) {
          throw recentAuthenticationRequired();
        }
        basementDid.pins.push(switchPin);
        return {};
      },
    },
  );
  openApp(path, accountHub({ servers: [basement], signedIn }).transport, undefined, () => fake);
  return basementDid;
}

async function setPin() {
  fireEvent.click(await findSettingsLink("Sign-in"));
  await screen.findByRole("button", { name: "Set Switch PIN" });
  fill("New Switch PIN", "2468");
  fireEvent.click(screen.getByRole("button", { name: "Set Switch PIN" }));
}

async function signInToHub() {
  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  fill("Email", ada.email);
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
}

test("a picked Server's sensitive change asks to sign in on the Hub again, then goes through", async () => {
  const basementDid = openBasement("/?settings=account");
  await screen.findByRole("combobox", { name: "Settings for" });
  pick("Settings for", "Basement");

  await setPin();
  const dialog = await screen.findByRole("alertdialog", { name: "Confirm it’s you" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Sign in again" }));
  await signInToHub();
  await setPin();

  await screen.findByText("Switch PIN set");
  expect(basementDid.pins).toStrictEqual(["2468"]);
});

test("signing in on the Hub trades for a new Device rather than keep the last one", async () => {
  localStorage.setItem(serverSessionKey(ada.id, basement.id), "kept from before");
  openBasement("/?settings=signin&manage=basement", false);

  await signInToHub();
  await screen.findByRole("button", { name: "Set Switch PIN" });

  expect(localStorage.getItem(serverSessionKey(ada.id, basement.id))).toBe("session 1");
});
