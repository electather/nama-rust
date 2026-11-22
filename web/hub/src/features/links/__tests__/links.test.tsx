import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { LinkService, LinkedServerSchema, LinkedUserSchema } from "@hub/gen/nama/hub/v1/link_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

const attic = create(LinkedServerSchema, {
  id: "0193-attic",
  name: "Attic",
  linkedAt: 1_700_000_000n,
});

const cabin = create(LinkedServerSchema, {
  id: "0193-cabin",
  name: "Cabin",
  linkedAt: 1_700_000_000n,
  url: "https://nama.example.com",
  addresses: ["https://nama.example.com", "http://192.168.1.10:7980"],
});

const grace = create(LinkedUserSchema, {
  id: "0195-grace",
  name: "Grace",
  serverName: "Cabin",
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
 * the code BDFH-JKLM, and its User Ida the code CDFG-HJKL, and entering either is refused for
 * `refusing`, if given. Ada's Account has Attic Linked, and Grace on Cabin, whose Admin set its
 * Server URL. With `listFails`, the Hub fails to list the Account's Servers.
 */
function openAccount(refusing?: RefusalReason, { listFails = false } = {}) {
  let session = true;
  const servers = [attic, cabin];
  let users = [grace];
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
      listLinkedServers: () => {
        if (listFails) {
          throw new ConnectError("the Hub failed", Code.Internal);
        }
        return { servers };
      },
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
      listLinkedUsers: () => ({ users }),
      listUserInvites: () => ({}),
      approveUserLink: ({ code }) => {
        if (refusing !== undefined) {
          throw refused(Code.AlreadyExists, refusing);
        }
        if (code !== "CDFG-HJKL") {
          throw refused(Code.NotFound, RefusalReason.LINK_CODE_NOT_FOUND);
        }
        const user = create(LinkedUserSchema, {
          id: "0196-ida",
          name: "Ida",
          serverName: "Living room",
          linkedAt: 1_700_000_100n,
        });
        users = [...users, user];
        return { user };
      },
      unlinkLinkedUser: ({ id }) => {
        users = users.filter((user) => user.id !== id);
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", "/");
  render(<RouterProvider router={getRouter(transport)} />);
}

/** Enters `code` in the form for a Server's code, or with `form` the one it names. */
async function enterCode(code: string, form = "Link a Server") {
  const entering = within(await screen.findByRole("form", { name: form }));
  fireEvent.change(entering.getByLabelText("Code"), { target: { value: code } });
  fireEvent.click(entering.getByRole("button", { name: /^Link (?:Server|User)$/u }));
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

  test("a Server's name links to its Server URL, if its Admin set one", async () => {
    openAccount();

    const servers = within(await screen.findByRole("region", { name: "Your Servers" }));
    const cabinLink = await servers.findByRole("link", { name: "Cabin" });

    expect(cabinLink.getAttribute("href")).toBe("https://nama.example.com");
    expect(servers.getByText("Attic").closest("a")).toBeNull();
  });

  test("a code no Server shows is said under the field", async () => {
    openAccount();

    await enterCode("XXXX-XXXX");

    await expect(
      screen.findByText(
        "Nothing is waiting on this code. Check it, or get a new one in the Server’s Settings.",
      ),
    ).resolves.toBeDefined();
  });

  test("an Account whose email isn't proven is offered a Sign-in Link to prove it", async () => {
    openAccount(RefusalReason.EMAIL_NOT_PROVEN);
    await enterCode("BDFH-JKLM");
    await expect(
      screen.findByText(
        "Linking needs your email proven first. Sign in with a Sign-in Link sent to it, then try again.",
      ),
    ).resolves.toBeDefined();

    fireEvent.click(screen.getByRole("link", { name: "Email me a Sign-in Link" }));

    await expect(
      screen.findByRole("heading", { name: "Email a Sign-in Link" }),
    ).resolves.toBeDefined();
  });

  test("a Server list the Hub can't give says so and leaves the rest of the page", async () => {
    openAccount(undefined, { listFails: true });

    await expect(
      screen.findByText(
        "The Hub couldn’t list your Servers just now. Reload the page to try again.",
      ),
    ).resolves.toBeDefined();
    expect(screen.getByRole("button", { name: "Link Server" })).toBeDefined();
  });

  test("a sign-in over 10 minutes old is offered signing in again", async () => {
    openAccount(RefusalReason.RECENT_AUTHENTICATION_REQUIRED);
    await enterCode("BDFH-JKLM");

    fireEvent.click(await screen.findByRole("button", { name: "Sign in again" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
  });
});

describe("linking a User", () => {
  beforeAll(() => {
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("the code a User's Server shows Links them, and they join the Account's Users", async () => {
    openAccount();
    await expect(screen.findByText("Grace on Cabin")).resolves.toBeDefined();

    await enterCode("CDFG-HJKL", "Link a User");

    await expect(
      screen.findByText("Ida on Living room is Linked to your Account."),
    ).resolves.toBeDefined();
    await expect(
      screen.findByText("Ida on Living room", { selector: "span" }),
    ).resolves.toBeDefined();
  });

  test("unlinking a User takes them off the Account's Users", async () => {
    openAccount();

    fireEvent.click(await screen.findByRole("button", { name: "Unlink Grace on Cabin" }));

    await expect(screen.findByText("No User is Linked yet.")).resolves.toBeDefined();
  });

  test("a User Linked to an Account already is said", async () => {
    openAccount(RefusalReason.USER_LINKED);

    await enterCode("CDFG-HJKL", "Link a User");

    await expect(
      screen.findByText(
        "This User is Linked to an Account already. Unlink them in their Server’s Settings first.",
      ),
    ).resolves.toBeDefined();
  });

  test("an Account whose email isn't proven is offered a Sign-in Link to prove it", async () => {
    openAccount(RefusalReason.EMAIL_NOT_PROVEN);

    await enterCode("CDFG-HJKL", "Link a User");

    await expect(
      screen.findByRole("link", { name: "Email me a Sign-in Link" }),
    ).resolves.toBeDefined();
  });
});
