import type { MessageInitShape } from "@bufbuild/protobuf";

import type { SetPlayedRequestSchema } from "@/gen/nama/v1/watch_state_pb";

/** What marking a card watched asks of the Server. */
export interface Marking {
  libraryId: bigint;
  /** The card's Movie, or its Episode, not the Show: the Show's next Episode takes its place. */
  target: NonNullable<MessageInitShape<typeof SetPlayedRequestSchema>["target"]>;
}
