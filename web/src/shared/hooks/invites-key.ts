import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the pending Invites, to refetch after making or revoking one. */
export function invitesKey(transport: Transport) {
  return createConnectQueryKey({
    schema: UserService.method.listInvites,
    cardinality: "finite",
    transport,
  });
}
