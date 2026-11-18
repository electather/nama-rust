import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";

import { serverFailedToPlan } from "@/features/player/lib/playback-wording";
import {
  PlaybackPlanSchema,
  PlaybackReasonKind,
  type PlaybackReasonSchema,
} from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { isCode } from "@/shared/lib/connect-errors";
import { refusalOf, serverMessage } from "@/shared/lib/forms";

/** A plan that can't play, saying why, for an `error` the player explains; any other is thrown. */
export function cantPlayBecause(error: unknown) {
  if (isCode(error, Code.Internal)) {
    return cantPlayPlan({ text: serverFailedToPlan });
  }
  const { reason, params } = refusalOf(error);
  const text = serverMessage(error);
  if (reason === RefusalReason.SEGMENT_FOLDER_UNWRITABLE) {
    return cantPlayPlan({ text });
  }
  if (reason !== RefusalReason.SERVER_BUSY) {
    throw error;
  }
  return cantPlayPlan({ kind: PlaybackReasonKind.SERVER_BUSY, params, text });
}

/** A plan that can't play, for the one `reason`. */
function cantPlayPlan(reason: MessageInitShape<typeof PlaybackReasonSchema>) {
  return create(PlaybackPlanSchema, { reasons: [{ ...reason, decided: true }] });
}
