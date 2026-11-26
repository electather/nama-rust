import { callUnaryMethod, createConnectQueryKey } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { bearer, forgetServerSessions } from "@/features/servers/lib/server-sessions";
import { type ListLinkedServersResponse, LinkService } from "@/gen/nama/hub/v1/link_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import type { ServerAt } from "@/shared/lib/server";

/**
 * Forgets the session this browser keeps on each of the Account's Servers, as Hub sign-in and
 * sign-out do, after running `leave`, such as leaving the pages that use them. Signs each Device
 * out on its Server too, without waiting, so a Server that never answers holds up nothing.
 */
export function useForgetServerSessions() {
  const queryClient = useQueryClient();
  const { transport: hub, serverAt } = useRouteContext({ from: "__root__" });
  return async (leave?: () => Promise<void> | void) => {
    // Read before `leave` may clear the cache: once signed out, the Hub lists no Servers.
    // ponytail: a Server neither cached nor on the signed-in Account's list keeps its Device until
    // it expires; keep each Server's address with its session if that matters.
    const listed = queryClient.getQueryData(
      createConnectQueryKey({
        schema: LinkService.method.listLinkedServers,
        input: {},
        transport: hub,
        cardinality: "finite",
      }),
    );
    await leave?.();
    const kept = forgetServerSessions();
    if (kept.length > 0) {
      const listServers = async (signal: AbortSignal) =>
        listed ??
        (await callUnaryMethod(hub, LinkService.method.listLinkedServers, {}, { signal }));
      void signOutKept(kept, listServers, serverAt);
    }
  };
}

// Nothing waits on these calls, but one the Hub or a Server never answers shouldn't stay open.
const signOutTimeoutMs = 10_000;

/** Signs each of `kept` out on its Server, at the address the Hub lists first, ignoring failures. */
async function signOutKept(
  kept: { serverId: string; token: string }[],
  listServers: (signal: AbortSignal) => Promise<ListLinkedServersResponse>,
  serverAt: ServerAt,
) {
  const signal = AbortSignal.timeout(signOutTimeoutMs);
  try {
    const { servers } = await listServers(signal);
    await Promise.allSettled(
      kept.map(async ({ serverId, token }) => {
        const [origin] = servers.find(({ id }) => id === serverId)?.addresses ?? [];
        if (origin !== undefined) {
          await callUnaryMethod(
            serverAt(origin, [bearer(token)]),
            SignInService.method.signOut,
            {},
            { signal },
          );
        }
      }),
    );
  } catch {
    // Best effort: a Device left signed in expires on its own.
  }
}
