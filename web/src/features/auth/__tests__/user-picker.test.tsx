import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";

afterEach(cleanup);

const PIN_TRIES = 5;

/**
 * A TV signed in as Sam, Ada and Kim, acting as `acting`, or 0n for none of them. Ada's Switch PIN is 1234. The Server's
 * other Users, such as Lee, sign in to it with "correct horse".
 */
function sharedTv(acting = 1n) {
  const revoked: [bigint, bigint | undefined][] = [];
  const tv = {
    acting,
    users: [
      { id: 1n, name: "Sam" },
      { id: 2n, name: "Ada" },
      { id: 3n, name: "Kim" },
    ],
    revoked,
    wrongPins: 0,
  };
  const actAs = (id: bigint) => {
    tv.acting = id;
    return {};
  };
  const transport = fakeServer(
    { signedIn: newUser },
    {
      whoAmI: () => {
        if (tv.users.length === 0) {
          throw new ConnectError("signed out", Code.Unauthenticated);
        }
        const user = tv.users.find((each) => each.id === tv.acting);
        return {
          user: user && { ...user, permissions: newUser },
          device: { id: 7n, name: "Living room TV", isCurrent: true },
          deviceUsers: tv.users,
        };
      },
      switchUser: ({ userId, switchPin }) => {
        if (!tv.users.some((each) => each.id === userId)) {
          throw refused(Code.NotFound, "no such User", { reason: RefusalReason.USER_NOT_FOUND });
        }
        if (userId !== 2n || switchPin === "1234") {
          return actAs(userId);
        }
        if (switchPin === "") {
          throw refused(Code.InvalidArgument, "a Switch PIN is needed", {
            reason: RefusalReason.FIELD_REQUIRED,
            field: "switch_pin",
          });
        }
        tv.wrongPins += 1;
        if (tv.wrongPins === PIN_TRIES) {
          tv.users = tv.users.filter((each) => each.id !== userId);
        }
        throw refused(Code.PermissionDenied, "wrong Switch PIN", {
          reason: RefusalReason.WRONG_SWITCH_PIN,
          field: "switch_pin",
          params: { triesLeft: String(PIN_TRIES - tv.wrongPins) },
        });
      },
      signOut: () => {
        tv.users = [];
        return {};
      },
      signIn: () => {
        tv.users = [...tv.users, { id: 4n, name: "Lee" }];
        return actAs(4n);
      },
    },
    {
      revokeDevice: ({ deviceId, userId }) => {
        tv.revoked.push([deviceId, userId]);
        tv.users = tv.users.filter((each) => each.id !== tv.acting);
        tv.acting = 0n;
        return {};
      },
    },
  );
  return { tv, transport };
}

/** Opens the menu of the User the app acts as, `name`, and picks `item` in it. */
async function fromMenu(name: string, item: string) {
  fireEvent.click(await screen.findByRole("button", { name: `${name}’s menu` }));
  fireEvent.click(await screen.findByRole("menuitem", { name: item }));
}

/** Expects the User picker to offer exactly `names`, in order. */
async function expectPicker(names: string[]) {
  const list = await screen.findByRole("list", { name: "Users on this Device" });
  const offered = within(list).getAllByRole("button");
  expect(offered).toHaveLength(names.length);
  for (const [at, name] of names.entries()) {
    expect(offered[at]).toBe(within(list).getByRole("button", { name }));
  }
}

test("the User picker lists only this Device's Users, and choosing one switches to them", async () => {
  const router = openApp("/code", sharedTv().transport);

  await fromMenu("Sam", "Switch User");
  await expectPicker(["Sam", "Ada", "Kim"]);
  fireEvent.click(screen.getByRole("button", { name: "Kim" }));

  await screen.findByRole("button", { name: "Kim’s menu" });
  expect(router.state.location.pathname).toBe("/code");
});

/** Opens the User picker, then picks Ada, whose Switch PIN it asks for. */
async function pickAda(transport: ReturnType<typeof sharedTv>["transport"]) {
  const router = openApp("/switch-user?redirect=%2Fcode", transport);
  fireEvent.click(await screen.findByRole("button", { name: "Ada" }));
  await screen.findByRole("heading", { name: "Enter Ada’s Switch PIN" });
  return router;
}

function enterPin(pin: string) {
  fill("Switch PIN", pin);
  fireEvent.click(screen.getByRole("button", { name: "Switch to Ada" }));
}

test("switching to a User with a Switch PIN asks for it, and a wrong one says how many tries are left", async () => {
  const router = await pickAda(sharedTv().transport);

  enterPin("0000");
  await expectUnderField("Switch PIN", "That isn’t Ada’s Switch PIN. 4 tries left.");
  enterPin("1234");

  await screen.findByRole("button", { name: "Ada’s menu" });
  expect(router.state.location.pathname).toBe("/code");
});

test("the last wrong Switch PIN says the User was signed out, and the picker no longer offers them", async () => {
  const { tv, transport } = sharedTv();
  tv.wrongPins = PIN_TRIES - 1;
  await pickAda(transport);

  enterPin("0000");
  await expect(
    screen.findByText(
      "That was the last try, so Ada was signed out of this Device. They can sign in again.",
    ),
  ).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("link", { name: "Pick another User" }));
  await expectPicker(["Sam", "Kim"]);
});

test("a User signed out of this Device elsewhere can't be switched to, and the picker drops them", async () => {
  const { tv, transport } = sharedTv();
  openApp("/switch-user", transport);
  await expectPicker(["Sam", "Ada", "Kim"]);
  tv.users = tv.users.filter((each) => each.id !== 3n);

  fireEvent.click(screen.getByRole("button", { name: "Kim" }));

  await expect(
    screen.findByText("That User isn’t signed in to this Device. They can sign in again."),
  ).resolves.toBeDefined();
  await expectPicker(["Sam", "Ada"]);
});

test("a Device acting as none of its Users opens the User picker, then the page", async () => {
  const router = openApp("/code", sharedTv(0n).transport);

  fireEvent.click(await screen.findByRole("button", { name: "Kim" }));

  await screen.findByRole("button", { name: "Kim’s menu" });
  expect(router.state.location.pathname).toBe("/code");
});

test("adding another User signs them in to this Device beside the others", async () => {
  openApp("/switch-user", sharedTv().transport);

  fireEvent.click(await screen.findByRole("link", { name: "Add another User" }));
  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  fill("Username", "Lee");
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  await expectPicker(["Sam", "Ada", "Kim", "Lee"]);
  expect(screen.getByRole("button", { name: "Lee" }).getAttribute("aria-current")).toBe("true");
});

test("signing out signs out only the User the Device acts as, and the picker offers the others", async () => {
  const { tv, transport } = sharedTv();
  openApp("/code", transport);

  await fromMenu("Sam", "Sign out");

  await expectPicker(["Ada", "Kim"]);
  // The caller's own User, not another's.
  expect(tv.revoked).toStrictEqual([[7n, undefined]]);
});

test("the User picker signs every User out of this Device", async () => {
  const { tv, transport } = sharedTv(0n);
  const router = openApp("/switch-user", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign everyone out" }));

  await screen.findByRole("heading", { name: "Sign in to Living Room" });
  expect(router.state.location.pathname).toBe("/sign-in");
  expect(tv.users).toStrictEqual([]);
});
