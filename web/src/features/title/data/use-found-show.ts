import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { type LookUpShowRequestSchema, RequestService } from "@/gen/nama/v1/request_pb";

/** A Show from the title source, for picking its Seasons to Request. */
export function useFoundShow(input: MessageInitShape<typeof LookUpShowRequestSchema>) {
  return useSuspenseQuery(RequestService.method.lookUpShow, input).data.show;
}
