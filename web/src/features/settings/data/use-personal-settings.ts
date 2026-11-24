import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the Personal settings, to refetch after saving or forgetting them. */
export function personalKey(transport: Transport) {
  return createConnectQueryKey({
    schema: UserService.method.listPersonalSettings,
    cardinality: "finite",
    transport,
  });
}

/** The signed-in User's own Personal settings, a card per Plugin that declares any. */
export function usePersonalSettings() {
  return useSuspenseQuery(UserService.method.listPersonalSettings);
}
