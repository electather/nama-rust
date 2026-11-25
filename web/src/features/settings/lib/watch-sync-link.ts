import { type StartWatchSyncLinkResponse, WatchSyncLinkState } from "@/gen/nama/v1/user_pb";

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
