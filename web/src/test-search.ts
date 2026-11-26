import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";

import type {
  HitSchema,
  HitsSchema,
  SearchResponseSchema,
  SearchService,
} from "@/gen/nama/v1/search_pb";

type Part = MessageInitShape<typeof SearchResponseSchema>;

/** A batch `Search` streams: its hits alone, or the whole `Hits`, such as the title source's. */
export type Batch = MessageInitShape<typeof HitSchema>[] | MessageInitShape<typeof HitsSchema>;

/**
 * A fake `Search` that streams each of `batches`, read anew for every search so it sees what a
 * test changed since, then `Done`, flagging the title source unavailable if `unavailable`.
 */
export function searchFinding(
  batches: () => Batch[],
  unavailable = false,
): ServiceImpl<typeof SearchService>["search"] {
  return () =>
    new ReadableStream<Part>({
      start(controller) {
        for (const batch of batches()) {
          const value = Array.isArray(batch) ? { hits: batch } : batch;
          controller.enqueue({ part: { case: "hits", value } });
        }
        controller.enqueue({
          part: { case: "done", value: { titleSourceUnavailable: unavailable } },
        });
        controller.close();
      },
    });
}
