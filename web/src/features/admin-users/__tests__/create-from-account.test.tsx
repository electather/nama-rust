import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { admin, fakeServer, openApp } from "@/test-app";
import { confirmItsYou, recentAuthenticationRequired } from "@/test-refusal";

afterEach(cleanup);

/** Settings' Users section on a Server Linked when `linked`, creating Users from Accounts with `create`. */
function openUsers(
  linked: boolean,
  create: NonNullable<Parameters<typeof fakeServer>[2]>["createUserFromAccount"] = () => ({
    id: 2n,
  }),
) {
  openApp(
    "/code?settings=users",
    fakeServer(
      {
        signedIn: admin,
        serverService: { getServerInfo: () => ({ name: "Living Room", accountSignIn: linked }) },
      },
      { reauthenticate: () => ({}) },
      { listUsers: () => ({}), createUserFromAccount: create },
    ),
  );
}

async function createFromAccount() {
  const form = await screen.findByRole("form", { name: "Create a User from an Account" });
  fireEvent.change(within(form).getByLabelText("Username"), { target: { value: "Grace" } });
  fireEvent.change(within(form).getByLabelText("Their Account’s email"), {
    target: { value: "grace@example.com" },
  });
  fireEvent.click(within(form).getByRole("button", { name: "Create from Account" }));
}

test("a Server that isn't Linked offers no User from an Account", async () => {
  openUsers(false);

  await screen.findByRole("button", { name: "Create User" });
  expect(screen.queryByRole("form", { name: "Create a User from an Account" })).toBeNull();
});

test("a User created from an Account signs in once it accepts on the Hub", async () => {
  const asked: { name: string; email: string }[] = [];
  openUsers(true, ({ name, email }) => {
    asked.push({ name, email });
    return { id: 2n };
  });

  await createFromAccount();

  await expect(
    screen.findByText("Grace can sign in once their Account accepts on the Hub."),
  ).resolves.toBeDefined();
  expect(asked).toStrictEqual([{ name: "Grace", email: "grace@example.com" }]);
});

test("while the Hub is down, no User is created and the Server says so", async () => {
  openUsers(true, () => {
    throw new ConnectError(
      "the Server couldn't ask the Hub, so no User was created; try again later",
      Code.Unavailable,
    );
  });

  await createFromAccount();

  await expect(
    screen.findByText("The Server couldn't ask the Hub, so no User was created; try again later."),
  ).resolves.toBeDefined();
});

/** Creates a User from an Account once the Device has Recent Authentication, refusing the first time. */
function oncePastRecentAuthentication() {
  let asked = false;
  return () => {
    if (!asked) {
      asked = true;
      throw recentAuthenticationRequired();
    }
    return { id: 2n };
  };
}

test("creating a User from an Account asks the Admin to confirm it's them first", async () => {
  openUsers(true, oncePastRecentAuthentication());

  await createFromAccount();
  await confirmItsYou();

  await expect(
    screen.findByText("Grace can sign in once their Account accepts on the Hub."),
  ).resolves.toBeDefined();
});
