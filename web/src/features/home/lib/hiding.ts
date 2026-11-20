import type { MessageInitShape } from "@bufbuild/protobuf";

import type { SetHiddenFromContinueWatchingRequestSchema } from "@/gen/nama/v1/watch_state_pb";

/** What removing a card from Continue Watching asks of the Server. */
export interface Hiding {
  libraryId: bigint;
  /** A Movie, or a Show by any of its Episodes. */
  target: NonNullable<
    MessageInitShape<typeof SetHiddenFromContinueWatchingRequestSchema>["target"]
  >;
}
