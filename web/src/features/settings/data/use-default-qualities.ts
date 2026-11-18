import { createConnectQueryKey, useQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** The query key of the User's default Qualities, to refetch after saving one. */
export const defaultsKey = createConnectQueryKey({
  schema: RequestService.method.listDefaultQualities,
  cardinality: "finite",
});

/** The signed-in User's own default Quality for each Fulfiller. */
export function useDefaultQualities() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the section shows nothing until it loads, and nothing if no Fulfiller has a Quality to pick
  return useQuery(RequestService.method.listDefaultQualities, {});
}
