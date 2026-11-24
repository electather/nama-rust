import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the notification preferences, to refetch after saving one. */
export function notificationPreferencesKey(transport: Transport) {
  return createConnectQueryKey({
    schema: UserService.method.listNotificationPreferences,
    cardinality: "finite",
    transport,
  });
}

/** The kinds of Request notification the signed-in User gets, each on or off. */
export function useNotificationPreferences() {
  return useSuspenseQuery(UserService.method.listNotificationPreferences, {}).data.preferences;
}
