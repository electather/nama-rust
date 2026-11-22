import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

type Users = Parameters<typeof fakeServer>[2];

function openSignin(users: Users) {
  openSettings("signin", fakeServer({ signedIn: newUser }, {}, users));
}

/** Where the User's Link stands, as the fake Server answers it. */
interface Shown {
  code?: string;
  url?: string;
  account?: string;
}

test("a User on a Server that can't Link Users sees no Hub section", async () => {
  openSignin({});

  await expect(screen.findByText("Sam")).resolves.toBeDefined();
  expect(screen.queryByRole("heading", { name: "Hub" })).toBeNull();
});

test("linking shows the code to enter on the Hub's form for Users", async () => {
  let link: Shown = {};
  openSignin({
    getUserLink: () => ({ link }),
    startUserLink: () => {
      link = { code: "CDFG-HJKL", url: "https://hub.example" };
      return { link };
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  await expect(screen.findByText("CDFG-HJKL")).resolves.toBeDefined();
  expect(screen.getByText(/enter this code under “Link a User”/u)).toBeDefined();
  expect(screen.getByRole("link", { name: "the Hub" }).getAttribute("href")).toBe(
    "https://hub.example",
  );
});

test("a Linked User names their Account, and Unlinking says they aren't Linked", async () => {
  let link: Shown = { account: "grace@example.com" };
  openSignin({
    getUserLink: () => ({ link }),
    unlinkUser: () => {
      link = {};
      return {};
    },
  });

  await expect(screen.findByText("grace@example.com")).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Unlink" }));

  await expect(screen.findByText("Not Linked")).resolves.toBeDefined();
});

test("a Hub the Server can't reach is said, and the User stays as they were", async () => {
  openSignin({
    getUserLink: () => ({ link: {} }),
    startUserLink: () => {
      throw new ConnectError("the Plugin that links the Server failed", Code.Unavailable);
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "Link to the Hub" }));

  const said = await screen.findByText("The Server couldn’t reach the Hub. Try again in a moment.");
  expect(said.getAttribute("role")).toBe("alert");
  expect(screen.getByText("Not Linked")).toBeDefined();
});
