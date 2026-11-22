import { Code, type Transport } from "@connectrpc/connect";
import { callUnaryMethod, createQueryOptions } from "@connectrpc/connect-query";
import { type QueryClient, queryOptions } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { isCode } from "@/shared/lib/connect-errors";

/** What every route's `beforeLoad` gets from the router. */
export interface RouterContext {
  queryClient: QueryClient;
  transport: Transport;
}

// src/routes/AGENTS.md, "Access checks per route".
// Both answers stay cached until signing in, signing out or setting up clears the cache. Saving the
// Server settings also resets the Server's info.

/** Whether the Server has no Users yet, so the first Admin still has to be created. */
export async function isSetupRequired(context: RouterContext) {
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

/** The query behind `whoIsSignedIn`: being signed out isn't an error. */
export function signedInQuery(transport: Transport) {
  return queryOptions({
    queryKey: ["signed-in"],
    queryFn: async () => {
      try {
        return await callUnaryMethod(transport, SignInService.method.whoAmI, {});
      } catch (error) {
        if (isCode(error, Code.Unauthenticated)) {
          return null;
        }
        throw error;
      }
    },
  });
}
