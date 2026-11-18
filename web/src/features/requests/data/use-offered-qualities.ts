import { useQuery } from "@connectrpc/connect-query";

import type { TitleKind } from "@/gen/nama/v1/match_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";

/** The Qualities the Fulfiller for `kind` offers. */
export function useOfferedQualities(kind: TitleKind) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the menu is an extra on a row, left out until it loads and when the Fulfiller offers no other
  return useQuery(RequestService.method.listOfferedQualities, { kind });
}
