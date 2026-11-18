import { createConnectQueryKey, useQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the Personal settings, to refetch after saving or forgetting them. */
export const personalKey = createConnectQueryKey({
  schema: UserService.method.listPersonalSettings,
  cardinality: "finite",
});

/** The signed-in User's own Personal settings, a card per Plugin that declares any. */
export function usePersonalSettings() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the section shows nothing until it loads, and nothing if no Plugin declares any
  return useQuery(UserService.method.listPersonalSettings);
}
