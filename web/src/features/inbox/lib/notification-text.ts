import type { InboxNotification } from "@/gen/nama/v1/inbox_pb";
import { NotificationKind } from "@/gen/nama/v1/user_pb";

/** What happened to the Request, after its title, for each kind of notification. */
const happened: Record<NotificationKind, string> = {
  [NotificationKind.UNSPECIFIED]: "changed",
  [NotificationKind.APPROVED]: "is approved",
  [NotificationKind.DECLINED]: "is declined",
  [NotificationKind.FAILED]: "Failed",
  [NotificationKind.PARTIALLY_AVAILABLE]: "is Partially Available",
  [NotificationKind.AVAILABLE]: "is Available",
  [NotificationKind.CANCELLED_BY_DELETION]: "is cancelled because its media was deleted",
  [NotificationKind.NEW_PENDING]: "needs approving",
};

/** What a notification says, such as "Severance Season 2 is approved". */
export function notificationText({ kind, title, seasonNumber }: InboxNotification) {
  const what = seasonNumber === undefined ? title : `${title} Season ${seasonNumber}`;
  return `${what} ${happened[kind]}`;
}
