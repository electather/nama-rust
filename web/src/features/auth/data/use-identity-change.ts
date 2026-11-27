import { Code, type Transport } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext, useRouter } from "@tanstack/react-router";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import {
  cachedLinkedServers,
  useForgetServerSessions,
} from "@/features/servers/data/use-forget-server-sessions";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { askAgainWhoIsSignedIn } from "@/shared/hooks/access";
import { leaveBrowserPush, moveBrowserPush } from "@/shared/hooks/browser-push";
import { isCode } from "@/shared/lib/connect-errors";

// `web/src/AGENTS.md`, "One module changes who is signed in".

/**
 * A change of who the web app is signed in as: the User a Device acts as, or the Account at the
 * Hub. "signed in" is to a Server, any way, another User added, an Invite joined or the Server set
 * up. A sign-in goes on to `goOn`, the path its call's answer names, or else Home. "account signed
 * out" is every "Sign in again" too, which goes on to `goOn` once signed in.
 */
export type IdentityChange<T> =
  | { kind: "signed in"; goOn: string | undefined | ((answer: T) => string) }
  | { kind: "switched"; goOn: string | undefined | ((answer: T) => string) }
  | { kind: "account signed in"; goOn: string | undefined | ((answer: T) => string) }
  | { kind: "account signed out"; goOn: string | undefined }
  | { kind: "account deleted" }
  | { kind: "user signed out" }
  | { kind: "device signed out" };

type ServerSignIn<T> = Extract<IdentityChange<T>, { kind: "signed in" | "switched" }>;

type AccountSignOut = Extract<
  IdentityChange<unknown>,
  { kind: "account signed out" | "account deleted" }
>;

type GoOn<T> = ServerSignIn<T>["goOn"];

/** The call that makes a change, at the Server or the Hub, or the answer of one already made. */
type Call<T> = (transport: Transport) => Promise<T> | T;

/**
 * Makes `change`: the function it returns takes the call that makes it, at the Server, or the Hub
 * for an Account, and runs it between the steps that need the old identity and the rest. A call
 * that fails runs nothing after it and rethrows, so its form shows the Refusal.
 */
export function useIdentityChange<T>(change: IdentityChange<T>) {
  const at = useAt();
  return async (call: (transport: Transport) => Promise<T> | T) => {
    if (change.kind === "signed in" || change.kind === "switched") {
      await signInToServer(at, change, call);
    } else if (change.kind === "user signed out") {
      await signOutOfServer(at, call, "/switch-user");
    } else if (change.kind === "device signed out") {
      await signOutOfServer(at, call, "/sign-in");
    } else if (change.kind === "account signed in") {
      await signInToAccount(at, change.goOn, call);
    } else {
      await signOutOfAccount(at, change, call);
    }
  };
}

/** What the steps change: the Server and the Hub, the cache, the page, and the kept sessions. */
function useAt() {
  return {
    server: useTransport(),
    // The Hub's, even from a Server's pages at the Hub, which run against that Server.
    hub: useRouteContext({ from: "__root__", select: ({ transport }) => transport }),
    queryClient: useQueryClient(),
    navigate: useNavigate(),
    router: useRouter(),
    forgetServerSessions: useForgetServerSessions(),
  };
}

type At = ReturnType<typeof useAt>;

async function signInToServer<T>(at: At, change: ServerSignIn<T>, call: Call<T>) {
  const answer = await callServer(at, change, call);
  // The Device now acts as this User (ADR 0106), and the Server keeps pushing whoever it was.
  await moveBrowserPush(at.server);
  // Forget the signed-out visitor, and anything cached for whoever was signed in before.
  at.queryClient.clear();
  await goOnReplacing(at, change.goOn, answer);
}

async function callServer<T>(at: At, change: ServerSignIn<T>, call: Call<T>) {
  try {
    return await call(at.server);
  } catch (error) {
    if (change.kind === "switched" && isCode(error, Code.NotFound)) {
      // Signed out of this Device elsewhere: asking again who is lets the User picker drop them.
      await askAgainWhoIsSignedIn(
        { queryClient: at.queryClient, transport: at.server },
        "device users",
        async () => {
          await at.router.invalidate();
        },
      );
    }
    throw error;
  }
}

async function signOutOfServer(at: At, call: Call<unknown>, to: "/switch-user" | "/sign-in") {
  // While still signed in: the Server unsubscribes only the User asking. A failure doesn't stop it.
  await leaveBrowserPush(at.server);
  await call(at.server);
  at.queryClient.clear();
  // The User picker goes on to sign-in once nobody is left (ADR 0106).
  await at.navigate({ to });
}

async function signInToAccount<T>(at: At, goOn: GoOn<T>, call: Call<T>) {
  const answer = await call(at.hub);
  const listed = cachedLinkedServers(at.queryClient, at.hub);
  at.queryClient.clear();
  // A Device kept from before lacks this sign-in's Recent Authentication; the next page's trade
  // has it (ADR 0105).
  at.forgetServerSessions(listed);
  await goOnReplacing(at, goOn, answer);
}

async function signOutOfAccount(at: At, change: AccountSignOut, call: Call<unknown>) {
  try {
    await call(at.hub);
  } catch (error) {
    // Already signed out, elsewhere or by expiring: only this browser's part is left to do.
    const signedOut = hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED;
    if (change.kind !== "account signed out" || !signedOut) {
      throw error;
    }
  }
  // Read before clearing: signed out, the Hub lists no Servers.
  const listed = cachedLinkedServers(at.queryClient, at.hub);
  at.queryClient.clear();
  const redirect = change.kind === "account signed out" ? change.goOn : undefined;
  await at.navigate({ to: "/sign-in", search: { redirect } });
  // Only once its pages are gone, so none signs in to a Server again.
  at.forgetServerSessions(listed);
}

/** Goes on to `goOn` in the sign-in page's place: going back to it would only send Back on again. */
async function goOnReplacing<T>(at: At, goOn: GoOn<T>, answer: T) {
  const href = typeof goOn === "function" ? goOn(answer) : (goOn ?? "/");
  await at.navigate({ href, replace: true });
}

/**
 * An `onError` for a call to the Hub that sends the Account to sign in, then back to this page,
 * when its session ended meanwhile, elsewhere or by expiring: trying again could never succeed.
 */
export function useSignInIfSessionEnded() {
  const queryClient = useQueryClient();
  const router = useRouter();
  const hub = useRouteContext({ from: "__root__", select: ({ transport }) => transport });
  // `src/router.tsx` leaves an ended session to the Hub's page: rerunning `_member`'s access check
  // sends the Account to sign in. The cache and kept sessions wait for that sign-in (#2573).
  return async (error: Error) => {
    if (hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
      await askAgainWhoIsSignedIn({ queryClient, transport: hub }, "account", async () => {
        await router.invalidate();
      });
    }
  };
}
