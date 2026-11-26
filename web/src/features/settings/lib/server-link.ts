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
  return error === null || isCode(error, Code.Canceled) ? undefined : userWording(error);
}

function userWording(error: ConnectError) {
  return refusalOf(error).reason === RefusalReason.LAST_SIGN_IN_METHOD
    ? "You sign in only with your Account, so Unlinking would leave you no way in. Ask an Admin to give you a password first."
    : unreachableHub;
}

const unreachableHub = "The Server couldn’t reach the Hub. Try again in a moment.";

/** What to say about Linking or Unlinking that failed; nothing for a cancelled "Confirm it’s you". */
export function linkFailure(error: ConnectError | null) {
  return error === null || isCode(error, Code.Canceled) ? undefined : wording(error);
}

function wording(error: ConnectError) {
  return linkWording[refusalOf(error).reason] ?? unreachableHub;
}

const linkWording: Partial<Record<RefusalReason, string>> = {
  [RefusalReason.NO_SERVER_LINKER]:
    "The Server’s config names no Hub, so it can’t be Linked. Name one in the config.",
  [RefusalReason.NOT_A_SERVER_ADDRESS]:
    "Open the Server at its Server URL to Link it this way, or show a code instead.",
};
