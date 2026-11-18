import { Code, type Transport } from "@connectrpc/connect";
import { callUnaryMethod, createQueryOptions } from "@connectrpc/connect-query";
import { type QueryClient, queryOptions } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { Permission, SignInService, type User } from "@/gen/nama/v1/sign_in_pb";
import { isCode } from "@/shared/lib/connect-errors";
import { may } from "@/shared/lib/permissions";

/** What every route's `beforeLoad` gets from the router. */
export interface RouterContext {
  queryClient: QueryClient;
  transport: Transport;
}

// src/routes/AGENTS.md, "Access checks per route".
// Both answers stay cached until signing in, signing out or setting up clears the cache.

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

/** The Admin pages, in the order the Admin nav lists them, each with the Permission it needs. */
export const adminPages = [
  { to: "/admin/libraries", label: "Libraries", permission: Permission.MANAGE_LIBRARIES },
  { to: "/admin/needs-a-match", label: "Needs a Match", permission: Permission.MANAGE_LIBRARIES },
  { to: "/admin/users", label: "Users", permission: Permission.MANAGE_USERS },
  { to: "/admin/plugins", label: "Plugins", permission: Permission.MANAGE_SETTINGS },
  { to: "/admin/playback", label: "Playback", permission: Permission.ADMIN },
] as const;

/** The Admin pages `user` may open. */
export function adminPagesFor(user: User | undefined) {
  return adminPages.filter(({ permission }) => may(user, permission));
}
