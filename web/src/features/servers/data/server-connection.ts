import { Code, ConnectError, type Interceptor, type Transport } from "@connectrpc/connect";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { queryOptions } from "@tanstack/react-query";

import { isBelowMinimumVersion } from "@/features/servers/lib/minimum-version";
import { type LinkedServer, LinkService } from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService, type WhoAmIResponse } from "@/gen/nama/v1/sign_in_pb";
import { serverKey } from "@/shared/hooks/server-key";
import { isCode } from "@/shared/lib/connect-errors";
import type { ServerAt } from "@/shared/lib/server";

/** How the page at the Hub reaches one of the Account's Servers. */
type ServerConnection =
  | { kind: "signed-in"; transport: Transport; signedIn: WhoAmIResponse }
  /** The Server is older than the page needs, so it opens in its own web app. */
  | { kind: "outdated" }
  /** The Account owns the Server but has no User on it, so there's nothing to browse. */
  | { kind: "no-user" };

/** What the Hub at `hub` needs to reach one of the Account's Servers. */
export interface ServerToReach {
  hub: Transport;
  server: LinkedServer;
  serverAt: ServerAt;
}

/**
 * Reaches `server`, signed in as the Account's User there, as a Device of its own whose session
 * this browser keeps (ADR 0105). Fails while the Server or the Hub can't be reached.
 */
export function serverConnectionQuery(reach: ServerToReach) {
  return queryOptions({
    queryKey: ["server-connection", serverKey(reach.hub), reach.server.id],
    queryFn: async () => {
      const connection = await connect(reach);
      return connection;
    },
    staleTime: "static",
    // The Server's queries are keyed by its transport, so a new one would lose them all.
    gcTime: Infinity,
    // Offline shows at once, rather than after the retries of a Server that may be off for days.
    retry: false,
  });
}

async function connect({ hub, server, serverAt }: ServerToReach): Promise<ServerConnection> {
  // shortcut: only the best Server Address is tried; try the next when one can't be reached.
  const [origin] = server.addresses;
  if (origin === undefined) {
    throw new ConnectError("the Server has no address", Code.Unavailable);
  }
  const open = serverAt(origin, []);
  const { version } = await callUnaryMethod(open, ServerService.method.getServerInfo, {});
  if (isBelowMinimumVersion(version)) {
    return { kind: "outdated" };
  }
  const session = new KeptSession(server.id, (token) => serverAt(origin, [bearer(token)]));
  // A Device revoked on the Server, or a session it forgot, is traded for a new one.
  const kept = await session.signIn();
  return kept ?? (await signInAnew({ hub, server, open, session }));
}

/** Trades the Hub's token for a new Device on the Server `open` reaches, keeping its session. */
async function signInAnew({
  hub,
  server,
  open,
  session,
}: {
  hub: Transport;
  server: LinkedServer;
  open: Transport;
  session: KeptSession;
}): Promise<ServerConnection> {
  const hubToken = await tokenFromHub(hub, server.id);
  if (hubToken === null) {
    return { kind: "no-user" };
  }
  const { token } = await callUnaryMethod(open, SignInService.method.signInWithHubToken, {
    hubToken,
  });
  session.keep(token);
  const signedIn = await session.signIn();
  if (!signedIn) {
    throw new ConnectError("the Server refused its own session", Code.Unauthenticated);
  }
  return signedIn;
}

/** The session this browser keeps for one Linked Server's Device, across page loads. */
class KeptSession {
  private readonly storageKey: string;
  private readonly withToken: (token: string) => Transport;

  public constructor(linkedServerId: string, withToken: (token: string) => Transport) {
    this.storageKey = `nama:server-session:v1:${linkedServerId}`;
    this.withToken = withToken;
  }

  public keep(token: string) {
    localStorage.setItem(this.storageKey, token);
  }

  /** Who the kept session is signed in as, or `null` with none or one the Server refuses. */
  public async signIn() {
    const token = localStorage.getItem(this.storageKey);
    if (token === null) {
      return null;
    }
    const signedIn = await signedInWith(this.withToken(token));
    return signedIn;
  }
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

/** Who `transport` is signed in as, or `null` when the Server refuses its session. */
async function signedInWith(transport: Transport) {
  try {
    const signedIn = await callUnaryMethod(transport, SignInService.method.whoAmI, {});
    return { kind: "signed-in", transport, signedIn } as const;
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
