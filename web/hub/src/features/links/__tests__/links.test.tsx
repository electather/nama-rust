import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { LinkService, LinkedServerSchema } from "@hub/gen/nama/hub/v1/link_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

const attic = create(LinkedServerSchema, {
  id: "0193-attic",
  name: "Attic",
  linkedAt: 1_700_000_000n,
});

/** A Connect error as the Hub refuses with one. */
function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason }) },
  ]);
}

/**
 * Opens the Account page, signed in as Ada, on a fake Hub where one Server, "Living room", shows
 * the code BDFH-JKLM, and entering it is refused for `refusing`, if given.
 */
function openAccount(refusing?: RefusalReason) {
  let session = true;
  const servers = [attic];
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { account: ada };
      },
      signOut: () => {
        session = false;
        return {};
      },
    });
    service(LinkService, {
      listLinkedServers: () => ({ servers }),
      approveServerLink: ({ code }) => {
        if (refusing !== undefined) {
          throw refused(Code.FailedPrecondition, refusing);
        }
        if (code.replace("-", "").toUpperCase() !== "BDFHJKLM") {
          throw refused(Code.NotFound, RefusalReason.LINK_CODE_NOT_FOUND);
        }
        const server = create(LinkedServerSchema, {
          id: "0194-living",
          name: "Living room",
          linkedAt: 1_700_000_100n,
        });
        servers.push(server);
        return { server };
      },
    });
  });
  globalThis.history.replaceState(null, "", "/");
  render(<RouterProvider router={getRouter(transport)} />);
}

async function enterCode(code: string) {
  fireEvent.change(await screen.findByLabelText("Code"), { target: { value: code } });
  fireEvent.click(screen.getByRole("button", { name: "Link Server" }));
}

describe("linking a Server", () => {
  beforeAll(() => {
    // A file's first `findBy…` waits while the route chunks load for the first time.
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("the code a Server shows Links it, and it joins the Account's Servers", async () => {
    openAccount();
    await expect(screen.findByText("Attic")).resolves.toBeDefined();

    await enterCode("bdfh-jklm");

    await expect(
      screen.findByText("Living room is Linked to your Account."),
    ).resolves.toBeDefined();
    await expect(screen.findByText("Living room", { selector: "span" })).resolves.toBeDefined();
  });

  test("a code no Server shows is said under the field", async () => {
    openAccount();

    await enterCode("XXXX-XXXX");

    await expect(
      screen.findByText(
        "No Server is showing this code. Check it, or get a new one in the Server’s Settings.",
      ),
    ).resolves.toBeDefined();
  });

  test("an Account whose email isn't proven is told to prove it with a Sign-in Link", async () => {
    openAccount(RefusalReason.EMAIL_NOT_PROVEN);

    await enterCode("BDFH-JKLM");

    await expect(
      screen.findByText(
        "Linking needs your email proven first. Sign in with a Sign-in Link sent to it, then enter the code again.",
      ),
    ).resolves.toBeDefined();
  });

  test("a sign-in over 10 minutes old is offered signing in again", async () => {
    openAccount(RefusalReason.RECENT_AUTHENTICATION_REQUIRED);
    await enterCode("BDFH-JKLM");

    fireEvent.click(await screen.findByRole("button", { name: "Sign in again" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
  });
});
