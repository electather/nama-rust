import { Code, ConnectError, type Interceptor, type Transport } from "@connectrpc/connect";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { queryOptions } from "@tanstack/react-query";

import { isBelowMinimumVersion } from "@/features/servers/lib/minimum-version";
import { serverSessionKey } from "@/features/servers/lib/server-sessions";
import { type LinkedServer, LinkService } from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService, type WhoAmIResponse } from "@/gen/nama/v1/sign_in_pb";
import { serverKey } from "@/shared/hooks/server-key";
import { isCode } from "@/shared/lib/connect-errors";
import type { ServerAt } from "@/shared/lib/server";

/** What the page at the Hub can do with one of the Account's Servers. */
type ServerReach =
  | {
      kind: "signed-in";
      transport: Transport;
      signedIn: WhoAmIResponse;
      /** The Settings Sections the Server serves, by their `?settings=` words. */
      settingsSections: string[];
    }
  /** The page can't browse the Server, so it opens in its own web app instead. */
  | { kind: "opens-alone"; why: OpensAloneWhy }
  /** The Account owns the Server but has no User on it, so there's nothing to browse. */
  | { kind: "no-user" };

/**
 * Why a Server opens in its own web app: it's older than the page needs, or it has no HTTPS
 * address the page may call.
 */
export type OpensAloneWhy = "outdated" | "plain-http";

/** What the page at the Hub `hub` needs to reach one of the signed-in Account's Servers. */
export interface ServerToReach {
  hub: Transport;
  accountId: string;
  server: LinkedServer;
  serverAt: ServerAt;
}

/**
 * Reaches `server`, signed in as the Account's User there, as a Device of its own whose session
 * this browser keeps (ADR 0105). Fails while the Server or the Hub can't be reached.
 */
export function serverReachQuery(reach: ServerToReach) {
  return queryOptions({
    queryKey: ["server-reach", serverKey(reach.hub), reach.accountId, reach.server.id],
    queryFn: async () => {
      const reached = await reachServer(reach);
      return reached;
    },
    staleTime: "static",
    // The Server's queries are keyed by its transport, so a new one would lose them all.
    gcTime: Infinity,
    // Offline shows at once, rather than after the retries of a Server that may be off for days.
    retry: false,
  });
}

async function reachServer(reach: ServerToReach): Promise<ServerReach> {
  // ponytail: only the best Server Address is tried; try the next when one can't be reached.
  const [origin] = reach.server.addresses;
  if (origin === undefined) {
    throw new ConnectError("the Server has no address", Code.Unavailable);
  }
  const open = reach.serverAt(origin, []);
  const info = await infoOrWhyAlone(open, origin);
  if (typeof info === "string") {
    return { kind: "opens-alone", why: info };
  }
  const storageKey = serverSessionKey(reach.accountId, reach.server.id);
  const signInWith = async (token: string) => {
    const at = reach.serverAt(origin, [bearer(token)]);
    const signedIn = await whoIsSignedInAt(at, info.settingsSections);
    return signedIn;
  };
  // A Device revoked on the Server, or a session it forgot, is traded for a new one.
  const kept = await signInKept(storageKey, signInWith);
  return (
    kept ??
    (await signInAnew(reach, open, async (token) => {
      localStorage.setItem(storageKey, token);
      const signedIn = await signInWith(token);
      return signedIn;
    }))
  );
}

/** What the Server `open` reaches at `origin` says of itself, or why it opens in its own web app. */
async function infoOrWhyAlone(open: Transport, origin: string) {
  if (blocksPlainHttp(origin)) {
    return "plain-http";
  }
  const info = await infoIfItAnswers(open, origin);
  return info && !isBelowMinimumVersion(info.version) ? info : "outdated";
}

/**
 * Whether the browser blocks the page's calls to `origin`: an HTTPS page's to plain HTTP, until the
 * Server has a certificate (ADR 0057, 0105).
 */
function blocksPlainHttp(origin: string) {
  return origin.startsWith("http:") && globalThis.location.protocol === "https:";
}

/** What the Server says of itself, or `null` when it answers but not to the Hub's origin. */
async function infoIfItAnswers(open: Transport, origin: string) {
  try {
    return await callUnaryMethod(open, ServerService.method.getServerInfo, {});
  } catch (error) {
    // A refused CORS preflight fails as an unreachable Server does, as a failed `fetch`; only a
    // request whose answer the page can't read tells them apart. One that answers is older than
    // the Hub's CORS (ADR 0105).
    if (isCode(error, Code.Unknown) && (await answersAt(origin))) {
      return null;
    }
    throw error;
  }
}

async function answersAt(origin: string) {
  try {
    await fetch(origin, { mode: "no-cors" });
    return true;
  } catch {
    return false;
  }
}

/** Signs in with the session kept at `storageKey`, or `null` with none or one the Server refuses. */
async function signInKept(
  storageKey: string,
  signInWith: (token: string) => Promise<ServerReach | null>,
) {
  const token = localStorage.getItem(storageKey);
  if (token === null) {
    return null;
  }
  const signedIn = await signInWith(token);
  return signedIn;
}

/** Trades the Hub's token for a new Device on the Server `open` reaches, then signs in with it. */
async function signInAnew(
  { hub, server }: ServerToReach,
  open: Transport,
  signInWith: (token: string) => Promise<ServerReach | null>,
): Promise<ServerReach> {
  const hubToken = await tokenFromHub(hub, server.id);
  if (hubToken === null) {
    return { kind: "no-user" };
  }
  const { token } = await callUnaryMethod(open, SignInService.method.signInWithHubToken, {
    hubToken,
  });
  const signedIn = await signInWith(token);
  if (!signedIn) {
    throw new ConnectError("the Server refused its own session", Code.Unauthenticated);
  }
  return signedIn;
}

/** The Hub's token for its Account's User on the Linked Server `linkedServerId`, or `null` for none. */
async function tokenFromHub(hub: Transport, linkedServerId: string) {
  try {
    const { token } = await callUnaryMethod(hub, LinkService.method.createServerToken, {
      linkedServerId,
    });
    return token;
  } catch (error) {
    const [refusal] = ConnectError.from(error).findDetails(RefusalSchema);
    if (refusal?.reason === RefusalReason.NO_USER_ON_SERVER) {
      return null;
    }
    throw error;
  }
}

/** Who `transport` is signed in as, on a Server serving `settingsSections`, or `null` if refused. */
async function whoIsSignedInAt(transport: Transport, settingsSections: string[]) {
  try {
    const signedIn = await callUnaryMethod(transport, SignInService.method.whoAmI, {});
    return { kind: "signed-in", transport, signedIn, settingsSections } as const;
  } catch (error) {
    if (isCode(error, Code.Unauthenticated)) {
      return null;
    }
    throw error;
  }
}

function bearer(token: string): Interceptor {
  return (next) => async (request) => {
    request.header.set("Authorization", `Bearer ${token}`);
    const response = await next(request);
    return response;
  };
}
