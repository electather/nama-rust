import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** The query key of the User's default Qualities, to refetch after saving one. */
export const defaultsKey = createConnectQueryKey({
  schema: RequestService.method.listDefaultQualities,
  cardinality: "finite",
});

/** The signed-in User's own default Quality for each Fulfiller with more than one to pick from. */
export function useDefaultQualities() {
  return useSuspenseQuery(RequestService.method.listDefaultQualities, {}).data.defaults;
}
