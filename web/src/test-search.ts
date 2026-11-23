import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";

import type { HitSchema, SearchResponseSchema, SearchService } from "@/gen/nama/v1/search_pb";

type Part = MessageInitShape<typeof SearchResponseSchema>;

/**
 * A fake `Search` that streams each of `batches`, read anew for every search so it sees what a
 * test changed since, then `Done`, flagging the title source unavailable if `unavailable`.
 */
export function searchFinding(
  batches: () => MessageInitShape<typeof HitSchema>[][],
  unavailable = false,
): ServiceImpl<typeof SearchService>["search"] {
  return () =>
    new ReadableStream<Part>({
      start(controller) {
        for (const hits of batches()) {
          controller.enqueue({ part: { case: "hits", value: { hits } } });
        }
        controller.enqueue({
          part: { case: "done", value: { titleSourceUnavailable: unavailable } },
        });
        controller.close();
      },
    });
}
