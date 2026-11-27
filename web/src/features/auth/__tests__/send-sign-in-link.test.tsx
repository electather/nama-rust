import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(cleanup);

test("signed out, the page asks for an email and offers signing in with a password", async () => {
  openApp("/send-sign-in-link", accountHub({ signedIn: false }).transport);

  const email = await screen.findByLabelText("Email");
  expect(email).toHaveProperty("value", "");
  expect(screen.getByRole("link", { name: "Sign in with a password" })).toBeDefined();
});

test("signed in, the email starts as the Account's, with no password link, and the Link goes to it", async () => {
  const { hub, transport } = accountHub();
  openApp("/send-sign-in-link", transport);

  const email = await screen.findByLabelText("Email");
  expect(email).toHaveProperty("value", "ada@example.com");
  expect(screen.queryByRole("link", { name: "Sign in with a password" })).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "Email me a Sign-in Link" }));

  await waitFor(() => {
    expect(hub.linksSentTo).toStrictEqual(["ada@example.com"]);
  });
});
