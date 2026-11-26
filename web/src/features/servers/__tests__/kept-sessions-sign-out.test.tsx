import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { serverSessionKey } from "@/features/servers/lib/server-sessions";
import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import type { ServerAt } from "@/shared/lib/server";
import { fill, openApp } from "@/test-app";
import { accountHub, ada } from "@/test-hub";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const servers = ["basement", "cabin"].map((id) =>
  create(LinkedServerSchema, { id, name: id, addresses: [`https://${id}.example`] }),
);

/**
 * Keeps a session on each of `servers`, one on a Server the Hub doesn't list, and one under a key
 * naming no Account, which a careless reader would take for the session on "basement".
 */
function keepSessions() {
  for (const { id } of [...servers, { id: "unlisted" }]) {
    localStorage.setItem(serverSessionKey(ada.id, id), `${id} token`);
  }
  localStorage.setItem("nama:server-session:v1:basement", "lost token");
}

function keptKeys() {
  return Object.keys(localStorage).filter((key) => key.startsWith("nama:server-session:"));
}

/**
 * Servers that note the origin and Authorization of each `SignOut` they get, then answer it, or
 * refuse or never answer it where `answer` says so for that origin.
 */
function serversSigningOut(answer: Record<string, "refuses" | "never answers">) {
  const signedOut: string[] = [];
  const serverAt: ServerAt = (origin, interceptors) =>
    createRouterTransport(
      ({ service }) => {
        service(SignInService, {
          signOut: async (_request, { requestHeader }) => {
            signedOut.push(`${origin} ${requestHeader.get("Authorization")}`);
            if (answer[origin] === "refuses") {
              throw new ConnectError("revoked", Code.Unauthenticated);
            }
            if (answer[origin] === "never answers") {
              await new Promise(() => {});
            }
            return {};
          },
        });
      },
      { transport: { interceptors } },
    );
  return { signedOut, serverAt };
}

const signedOutBoth = new Set([
  "https://basement.example Bearer basement token",
  "https://cabin.example Bearer cabin token",
]);

test("signing the Account out signs each kept Device out on its Server, despite one refusing", async () => {
  keepSessions();
  const { signedOut, serverAt } = serversSigningOut({ "https://basement.example": "refuses" });
  const { hub, transport } = accountHub({ servers });
  openApp("/?settings=account", transport, undefined, serverAt);

  const signOut = await screen.findByRole("button", { name: "Sign out" });
  // Signed out, the Hub lists no Servers, so only the list the page already has can name them.
  hub.servers = [];
  fireEvent.click(signOut);

  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  await waitFor(() => {
    expect(new Set(signedOut)).toStrictEqual(signedOutBoth);
  });
  expect(keptKeys()).toStrictEqual([]);
});

test("signing in at the Hub signs out the Devices kept from before, despite one never answering", async () => {
  keepSessions();
  const { signedOut, serverAt } = serversSigningOut({
    "https://basement.example": "never answers",
  });
  openApp("/sign-in", accountHub({ servers, signedIn: false }).transport, undefined, serverAt);

  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  fill("Email", ada.email);
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  await waitFor(() => {
    expect(new Set(signedOut)).toStrictEqual(signedOutBoth);
  });
  expect(keptKeys()).toStrictEqual([]);
});
