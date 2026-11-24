import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** The query key of the User's default Qualities, to refetch after saving one. */
export function defaultsKey(transport: Transport) {
  return createConnectQueryKey({
    schema: RequestService.method.listDefaultQualities,
    cardinality: "finite",
    transport,
  });
}

/** The signed-in User's own default Quality for each Fulfiller with more than one to pick from. */
export function useDefaultQualities() {
  return useSuspenseQuery(RequestService.method.listDefaultQualities, {}).data.defaults;
}
