import type { MessageInitShape } from "@bufbuild/protobuf";

import type { SetHiddenFromContinueWatchingRequestSchema } from "@/gen/nama/v1/watch_state_pb";

/** A card's title, keyed as the row lists it, and what hiding it from Continue Watching asks. */
export interface Hiding {
  key: string;
  title: string;
  libraryId: bigint;
  /** A Movie, or a Show by any of its Episodes. */
  target: NonNullable<
    MessageInitShape<typeof SetHiddenFromContinueWatchingRequestSchema>["target"]
  >;
}
