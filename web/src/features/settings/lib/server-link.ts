import { Code, type ConnectError } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { ServerLink } from "@/gen/nama/v1/server_pb";
import { isCode } from "@/shared/lib/connect-errors";
import { refusalOf } from "@/shared/lib/forms";

/** Where the Server's or a User's Link stands, as both messages hold it. */
export type ShownLink = Pick<ServerLink, "code" | "url" | "account">;

/** How often to look again while a code shows, which is when the Server asks the Hub too. */
export const WAITING_POLL_MS = 5000;

/** What to say about a User's Linking or Unlinking that failed; nothing for a cancelled "Confirm it’s you". */
export function userLinkFailure(error: ConnectError | null) {
  if (error === null || isCode(error, Code.Canceled)) {
    return undefined;
  }
  return refusalOf(error).reason === RefusalReason.NO_SIGN_IN_METHOD
    ? "You sign in only with your Account, so Unlinking would leave you no way in. Ask an Admin to give you a password first."
    : "The Server couldn’t reach the Hub. Try again in a moment.";
}

/** What to say about Linking or Unlinking that failed; nothing for a cancelled "Confirm it’s you". */
export function linkFailure(error: ConnectError | null) {
  return error === null || isCode(error, Code.Canceled) ? undefined : wording(error);
}

function wording(error: ConnectError) {
  return refusalOf(error).reason === RefusalReason.NO_SERVER_LINKER
    ? "The Hub Plugin isn’t enabled any more. Enable it under Plugins."
    : "The Server couldn’t reach the Hub. Try again; the Hub Plugin’s page under Plugins says why.";
}
