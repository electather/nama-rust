import { createConnectQueryKey } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the pending Invites, to refetch after making or revoking one. */
export const invitesKey = createConnectQueryKey({
  schema: UserService.method.listInvites,
  cardinality: "finite",
});
