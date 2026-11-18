import { createConnectQueryKey } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of every User's Devices, to refetch after a change that signs some out. */
export const devicesKey = createConnectQueryKey({
  schema: UserService.method.listDevices,
  cardinality: "finite",
});
