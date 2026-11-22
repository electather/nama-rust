import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import {
  type LinkedUser,
  LinkService,
  LinkedUserSchema,
  UserInviteSchema,
} from "@hub/gen/nama/hub/v1/link_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

const grace = create(UserInviteSchema, {
  id: "0197-grace",
  name: "Grace",
  serverName: "Cabin",
  invitedAt: 1_700_000_000n,
});

function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason }) },
  ]);
}

/** What the fake Hub answers: signed in or not, and refusing to accept for `refusing`, if given. */
interface Hub {
  signedIn?: boolean;
  refusing?: RefusalReason;
  /** Ada's User on the Server the sign-in is for: none if empty, and no such sign-in if undefined. */
  userOnServer?: string;
}

/** Opens the Hub at `path`, as Ada, with Grace on Cabin waiting for her to accept. */
function openHub(path: string, hub: Hub = {}) {
  let invites = [grace];
  let users: LinkedUser[] = [];
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => {
        if (hub.signedIn === false) {
          throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { account: ada };
      },
    });
    service(LinkService, {
      listLinkedServers: () => ({}),
      listLinkedUsers: () => ({ users }),
      listUserInvites: () => ({ invites }),
      acceptUserInvite: ({ id }) => {
        if (hub.refusing !== undefined) {
          throw refused(Code.FailedPrecondition, hub.refusing);
        }
        invites = invites.filter((invite) => invite.id !== id);
        const user = create(LinkedUserSchema, {
          id: "0199-grace-link",
          name: grace.name,
          serverName: grace.serverName,
          linkedAt: 1_700_000_100n,
        });
        users = [user];
        return { user };
      },
      getServerSignIn: ({ id }) => {
        if (hub.userOnServer === undefined || id !== "0198-sign-in") {
          throw refused(Code.NotFound, RefusalReason.SERVER_SIGN_IN_NOT_FOUND);
        }
        return { serverName: "Cabin", userName: hub.userOnServer };
      },
      approveServerSignIn: () => ({ returnUrl: "https://cabin.example/sign-in/account" }),
    });
  });
  globalThis.history.replaceState(null, "", path);
  const router = getRouter(transport);
  render(<RouterProvider router={router} />);
  return router;
}

beforeAll(() => {
  // A file's first `findBy…` waits while the route chunks load for the first time.
  configure({ asyncUtilTimeout: 5000 });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("accepting a User created from the Account Links them", async () => {
  openHub("/");

  fireEvent.click(await screen.findByRole("button", { name: "Accept Grace on Cabin" }));

  await expect(
    screen.findByRole("button", { name: "Unlink Grace on Cabin" }),
  ).resolves.toBeDefined();
  await vi.waitFor(() => {
    expect(screen.queryByRole("heading", { name: "Users waiting for you" })).toBeNull();
  });
});

test("an Account whose email isn't proven is offered a Sign-in Link before accepting", async () => {
  openHub("/", { refusing: RefusalReason.EMAIL_NOT_PROVEN });

  fireEvent.click(await screen.findByRole("button", { name: "Accept Grace on Cabin" }));

  await expect(
    screen.findByText(
      "Linking needs your email proven first. Sign in with a Sign-in Link sent to it, then try again.",
    ),
  ).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Email me a Sign-in Link" })).toBeDefined();
});

test("approving a sign-in to a Server names the User, then goes back to the Server", async () => {
  // Going to another site would end the test's page, so where it went is only noted.
  const went: string[] = [];
  vi.spyOn(globalThis.location, "assign").mockImplementation((url) => {
    went.push(String(url));
  });
  openHub("/server-sign-in/0198-sign-in", { userOnServer: "Grace" });

  await expect(screen.findByRole("heading", { name: "Sign in to Cabin" })).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Sign in as Grace" }));

  await vi.waitFor(() => {
    expect(went).toStrictEqual(["https://cabin.example/sign-in/account"]);
  });
});

test("an Account with no User on the Server is told to ask for one", async () => {
  openHub("/server-sign-in/0198-sign-in", { userOnServer: "" });

  await expect(
    screen.findByText(
      "Your Account has no User on this Server. Ask whoever runs it to create one from your Account.",
    ),
  ).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: /^Sign in as/u })).toBeNull();
});

test("an expired sign-in says to start again on the Server", async () => {
  openHub("/server-sign-in/0198-sign-in");

  await expect(
    screen.findByText("This sign-in expired or was used. Start signing in again on your Server."),
  ).resolves.toBeDefined();
});

test("signed out, the sign-in to a Server asks to sign in first and comes back", async () => {
  const router = openHub("/server-sign-in/0198-sign-in", {
    signedIn: false,
    userOnServer: "Grace",
  });

  await screen.findByRole("heading", { name: "Sign in" });
  expect(router.state.location.search.continue).toBe("/server-sign-in/0198-sign-in");
});
