import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";
import { may } from "@/shared/lib/permissions";

/** One of the Account's Servers where its User is an Admin, signed in there. */
export type AdminServer = ReturnType<typeof useAdminServers>[number];

/**
 * The signed-in Account's Servers where its User is an Admin, each as it's reached, so a slow
 * Server holds up none of the rest (ADR 0105).
 */
export function useAdminServers() {
  return useReachedServers(useLinkedServers()).reached.filter(({ signedIn }) =>
    may(signedIn.user, Permission.ADMIN),
  );
}
