import { pendingCountQuery } from "@/features/requests/data/use-pending-count";
import type { StateFilter } from "@/features/requests/lib/state-filter";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import type { RouterContext } from "@/shared/hooks/access";
import { may } from "@/shared/lib/permissions";

/** The filter the Requests page opens on: Pending for a Request manager while anything is, else All. */
export async function defaultFilter(
  { queryClient, transport }: RouterContext,
  user: User | undefined,
): Promise<StateFilter> {
  if (!may(user, Permission.MANAGE_REQUESTS)) {
    return "all";
  }
  const { count } = await queryClient.query(pendingCountQuery(transport));
  return count > 0 ? "pending" : "all";
}
