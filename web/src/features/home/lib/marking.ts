import type { MessageInitShape } from "@bufbuild/protobuf";

import type { SetPlayedRequestSchema } from "@/gen/nama/v1/watch_state_pb";

/** A card's title, keyed as the shelf lists it, and what marking it watched asks of the Server. */
export interface Marking {
  key: string;
  title: string;
  libraryId: bigint;
  /** The card's Movie, or its Episode, not the Show: the Show's next Episode takes its place. */
  target: NonNullable<MessageInitShape<typeof SetPlayedRequestSchema>["target"]>;
}
