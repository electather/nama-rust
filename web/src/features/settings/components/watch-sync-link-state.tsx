import { ServerLinkRow } from "@/features/settings/components/server-link-row";
import { WatchSyncCode } from "@/features/settings/components/watch-sync-code";
import type { LinkView } from "@/features/settings/lib/watch-sync-link";
import type { StartWatchSyncLinkResponse } from "@/gen/nama/v1/user_pb";

const notLinked = {
  says: <span className="text-muted-foreground">Not linked</span>,
  action: "Link your account",
};

/** What the row says, and its button, while no code shows: a code is waited on only once started. */
const offers = {
  idle: notLinked,
  code: notLinked,
  expired: {
    says: "The code expired or was declined. Get a new one to try again.",
    action: "Get a new code",
  },
};

/** Where linking an account at a tracker stands, with the one button that changes it: Link, Cancel or Unlink. */
export function WatchSyncLinkState({
  view,
  started,
  tracker,
  pending,
  onStart,
  onCancel,
  onUnlink,
}: {
  view: LinkView;
  started: StartWatchSyncLinkResponse | undefined;
  /** The Plugin's name, such as Trakt. */
  tracker: string;
  pending: boolean;
  onStart: () => void;
  onCancel: () => void;
  onUnlink: () => void;
}) {
  if (view === "code" && started !== undefined) {
    return <WatchSyncCode started={started} tracker={tracker} onCancel={onCancel} />;
  }
  if (view === "linked") {
    return (
      <ServerLinkRow
        says="Your account is linked"
        action="Unlink"
        doing="Unlinking…"
        variant="ghost"
        pending={pending}
        onClick={onUnlink}
      />
    );
  }
  const offer = offers[view];
  return (
    <ServerLinkRow
      says={offer.says}
      action={offer.action}
      doing="Getting a code…"
      variant="secondary"
      pending={pending}
      onClick={onStart}
    />
  );
}
