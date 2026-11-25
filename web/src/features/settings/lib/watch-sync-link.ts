import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type StartWatchSyncLinkResponse, WatchSyncLinkState } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/shared/lib/forms";

/** What linking an account at a Watch sync Plugin's tracker shows. */
export type LinkView = "idle" | "code" | "expired" | "linked";

/** What a poll's answer shows. */
const polledView: Record<WatchSyncLinkState, LinkView> = {
  [WatchSyncLinkState.UNSPECIFIED]: "code",
  [WatchSyncLinkState.PENDING]: "code",
  [WatchSyncLinkState.LINKED]: "linked",
  [WatchSyncLinkState.EXPIRED]: "expired",
};

/**
 * What to show, from whether a code was `started`, what the last poll of it said, and whether the
 * Personal settings say the account is `linked`.
 */
export function linkView(started: boolean, polled: WatchSyncLinkState, linked: boolean): LinkView {
  if (!started) {
    return linked ? "linked" : "idle";
  }
  return polledView[polled] ?? "code";
}

/** How long to wait between polls of the code `started` gave, as the tracker allows: a second at least. */
export function pollEveryMs(started: StartWatchSyncLinkResponse | undefined) {
  return Math.max(started?.intervalSeconds ?? 0, 1) * 1000;
}

/** Why linking or unlinking the account at `tracker` failed, by whichever of `linking` and `unlinking` did. */
export function linkFailed(linking: unknown, unlinking: unknown, tracker: string) {
  const [error, attempt] =
    linking === null
      ? [unlinking, `unlink your ${tracker} account`]
      : [linking, `link your ${tracker} account`];
  const reached = `The Server couldn’t reach ${tracker}. Try again in a moment.`;
  const gone = "This Plugin isn’t on the Server any more.";
  return error === null
    ? undefined
    : refusal(
        error,
        attempt,
        { [Code.Unavailable]: { form: reached } },
        { [RefusalReason.PLUGIN_NOT_FOUND]: { form: gone } },
      ).form;
}
