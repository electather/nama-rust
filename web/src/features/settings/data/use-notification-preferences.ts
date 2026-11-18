import { createConnectQueryKey, useQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the notification preferences, to refetch after saving one. */
export const notificationPreferencesKey = createConnectQueryKey({
  schema: UserService.method.listNotificationPreferences,
  cardinality: "finite",
});

/** The kinds of Request notification the signed-in User gets, each on or off. */
export function useNotificationPreferences() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the section shows nothing until it loads
  return useQuery(UserService.method.listNotificationPreferences, {});
}
