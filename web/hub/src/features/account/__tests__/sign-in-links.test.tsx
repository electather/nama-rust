import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import {
  type Account,
  AccountSchema,
  AccountService,
  type SendSignInLinkRequest,
} from "@hub/gen/nama/hub/v1/account_pb";
import { HubService } from "@hub/gen/nama/hub/v1/hub_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });
const bea = create(AccountSchema, { id: "0192-bea", email: "bea@example.com" });

/** The token of Ada's waiting Sign-in Link on the fake Hub. */
const token = "ada-token";

function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason }) },
  ]);
}

/**
 * Opens `path` on a fake Hub that emails Sign-in Links when `signInLinks` says so, with `signedIn`
 * signed in, if anyone, and one waiting link for Ada's unproven Account. Returns what was asked to
 * be sent, whether the Hub has said if it emails Sign-in Links, and how often a link was looked up.
 */
function openHub(path: string, signInLinks: boolean, signedIn?: Account) {
  let session = signedIn;
  let waiting = true;
  const sent: SendSignInLinkRequest[] = [];
  let toldSignInLinks = false;
  let linkLookUps = 0;
  const transport = createRouterTransport(({ service }) => {
    service(HubService, {
      getHubInfo: () => {
        toldSignInLinks = true;
        return { version: "0.1.0", signInLinks };
      },
    });
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { account: session };
      },
      sendSignInLink: (request) => {
        sent.push(request);
        return {};
      },
      getSignInLink: (request) => {
        linkLookUps += 1;
        if (!waiting || request.token !== token) {
          throw refused(Code.NotFound, RefusalReason.SIGN_IN_LINK_EXPIRED);
        }
        return { account: ada, emailProven: false };
      },
      signInWithLink: () => {
        waiting = false;
        session = ada;
        return { account: ada, continueTo: "/" };
      },
    });
  });
  globalThis.history.replaceState(null, "", path);
  render(<RouterProvider router={getRouter(transport)} />);
  return { sent, toldSignInLinks: () => toldSignInLinks, linkLookUps: () => linkLookUps };
}

describe("a Sign-in Link", () => {
  beforeAll(() => {
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("the sign-in page offers one, for a forgotten password, only when the Hub emails them", async () => {
    const hub = openHub("/sign-in", false);
    await screen.findByRole("heading", { name: "Sign in" });
    await waitFor(() => {
      expect(hub.toldSignInLinks()).toBe(true);
    });
    expect(screen.queryByRole("link", { name: "Email me a Sign-in Link" })).toBeNull();
    expect(screen.queryByText(/Forgot your password/u)).toBeNull();
    cleanup();

    openHub("/sign-in", true);

    await expect(
      screen.findByRole("link", { name: "Email me a Sign-in Link" }),
    ).resolves.toBeDefined();
    expect(screen.getByText(/Forgot your password/u)).toBeDefined();
  });

  test("asking for one says to check the inbox, and sends where the sign-in continues", async () => {
    const { sent } = openHub("/send-sign-in-link?continue=%2Fservers", true);
    await screen.findByRole("heading", { name: "Email a Sign-in Link" });

    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "ada@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Email me a Sign-in Link" }));

    await expect(screen.findByRole("heading", { name: "Check your email" })).resolves.toBeDefined();
    expect(sent.map(({ email, continueTo }) => ({ email, continueTo }))).toStrictEqual([
      { email: "ada@example.com", continueTo: "/servers" },
    ]);
  });

  test("opening one names the Account, says it takes it over, and signs in on the button", async () => {
    const hub = openHub(`/sign-in-link#${token}`, true);

    await expect(screen.findByText("ada@example.com")).resolves.toBeDefined();
    expect(screen.getByText(/removes its password/u)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    await expect(screen.findByRole("heading", { name: "Your Account" })).resolves.toBeDefined();
    // Looking the used link up again would have shown it as expired on the way.
    expect(hub.linkLookUps()).toBe(1);
  });

  test("opened while another Account is signed in, it says signing in signs that one out", async () => {
    openHub(`/sign-in-link#${token}`, true, bea);

    await expect(screen.findByText("bea@example.com")).resolves.toBeDefined();
    expect(screen.getByText(/Signing in here signs you out of it/u)).toBeDefined();
  });

  test("an expired or used one offers a new one", async () => {
    openHub("/sign-in-link#used-token", true);

    await expect(
      screen.findByRole("heading", { name: "This Sign-in Link has expired" }),
    ).resolves.toBeDefined();
    expect(screen.getByRole("link", { name: "Email me a new Sign-in Link" })).toBeDefined();
  });
});
