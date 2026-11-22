import { Code, type ConnectError } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { isCode } from "@/shared/lib/connect-errors";
import { refusalOf } from "@/shared/lib/forms";

/** What to say about Linking or Unlinking that failed; nothing for a cancelled "Confirm it’s you". */
export function linkFailure(error: ConnectError | null) {
  return error === null || isCode(error, Code.Canceled) ? undefined : wording(error);
}

function wording(error: ConnectError) {
  return refusalOf(error).reason === RefusalReason.NO_SERVER_LINKER
    ? "The Hub Plugin isn’t enabled any more. Enable it under Plugins."
    : "The Server couldn’t reach the Hub. Try again; the Hub Plugin’s page under Plugins says why.";
}
