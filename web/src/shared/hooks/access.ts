import { Code, type Transport } from "@connectrpc/connect";
import { callUnaryMethod, createQueryOptions } from "@connectrpc/connect-query";
import { type QueryClient, queryOptions } from "@tanstack/react-query";
import { redirect } from "@tanstack/react-router";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { HubService } from "@/gen/nama/hub/v1/hub_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { isCode } from "@/shared/lib/connect-errors";

/** What every route's `beforeLoad` gets from the router. */
export interface RouterContext {
  queryClient: QueryClient;
  transport: Transport;
}

// src/routes/AGENTS.md, "Access checks per route".
// Every answer here stays cached until signing in, signing out or setting up clears the cache. Saving
// the Server settings also resets the Server's info.

/**
 * Whether the Server has no Users yet, so the first Admin still has to be created. The Hub never
 * needs setup.
 */
export async function isSetupRequired(context: RouterContext) {
  if (await isAtHub(context)) {
    return false;
  }
  const info = await serverInfo(context);
  return info.setupRequired;
}

/** What any client can read about the Server, such as its name. */
export async function serverInfo({ queryClient, transport }: RouterContext) {
  const info = await queryClient.query({
    ...createQueryOptions(ServerService.method.getServerInfo, {}, { transport }),
    staleTime: "static",
  });
  return info;
}

/** Who is signed in on this Device, or `null` when nobody is. */
export async function whoIsSignedIn({ queryClient, transport }: RouterContext) {
  const signedIn = await queryClient.query({ ...signedInQuery(transport), staleTime: "static" });
  return signedIn;
}

/** Whether anyone is signed in where the app runs: an Account at the Hub, else a Server's User. */
export async function isSignedInHere(context: RouterContext) {
  const signedIn = (await isAtHub(context))
    ? await signedInAccount(context)
    : await whoIsSignedIn(context);
  return signedIn !== null;
}

/** The query behind `whoIsSignedIn`: being signed out isn't an error. */
export function signedInQuery(transport: Transport) {
  return queryOptions({
    queryKey: ["signed-in"],
    queryFn: async () => {
      const answer = await nullWhen(
        Code.Unauthenticated,
        callUnaryMethod(transport, SignInService.method.whoAmI, {}),
      );
      return answer;
    },
  });
}

/**
 * Whether the app runs at the Hub rather than at a Server (ADR 0105): only the Hub answers its
 * API, and a Server answers an RPC it doesn't serve with a 404, `unimplemented`.
 */
export async function isAtHub(context: RouterContext) {
  return (await hubInfo(context)) !== null;
}

/** What anyone can read about the Hub, such as whether it emails Sign-in Links; `null` at a Server. */
export async function hubInfo({ queryClient, transport }: RouterContext) {
  const info = await queryClient.query({
    queryKey: ["hub-info"],
    queryFn: async () => {
      const answer = await nullWhen(
        Code.Unimplemented,
        callUnaryMethod(transport, HubService.method.getHubInfo, {}),
      );
      return answer;
    },
    staleTime: "static",
  });
  return info;
}

/** The Account signed in at the Hub, or `null` when none is. */
export async function signedInAccount({ queryClient, transport }: RouterContext) {
  const signedIn = await queryClient.query({
    queryKey: signedInAccountKey,
    // The Hub refuses with `SIGN_IN_REQUIRED`, the only refusal `GetAccount` has.
    queryFn: async () => {
      const answer = await nullWhen(
        Code.Unauthenticated,
        callUnaryMethod(transport, AccountService.method.getAccount, {}),
      );
      return answer;
    },
    staleTime: "static",
  });
  return signedIn?.account ?? null;
}

/**
 * Forgets which Account is signed in at the Hub, for when a page finds its session ended, so the
 * next access check asks the Hub again.
 */
export function forgetSignedInAccount(queryClient: QueryClient) {
  queryClient.removeQueries({ queryKey: signedInAccountKey });
}

const signedInAccountKey = ["signed-in-account"];

/** The Account signed in at the Hub; without one, sends the visitor to sign in, then to `href`. */
export async function accountOrSignIn(context: RouterContext, href: string) {
  const account = await signedInAccount(context);
  if (!account) {
    throw redirect({ to: "/sign-in", search: { redirect: href } });
  }
  return account;
}

/** What `call` answers, or `null` when it fails with `code`, which here means "no", not an error. */
async function nullWhen<T>(code: Code, call: Promise<T>) {
  try {
    return await call;
  } catch (error) {
    if (isCode(error, code)) {
      return null;
    }
    throw error;
  }
}
