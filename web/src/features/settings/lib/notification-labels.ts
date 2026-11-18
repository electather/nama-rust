import { NotificationKind } from "@/gen/nama/v1/user_pb";

/** What each kind of notification tells the User, as its checkbox says it. */
export const notificationLabels: Record<NotificationKind, string> = {
  [NotificationKind.UNSPECIFIED]: "",
  [NotificationKind.APPROVED]: "Your Request is approved",
  [NotificationKind.DECLINED]: "Your Request is declined",
  [NotificationKind.FAILED]: "Your Request Failed",
  [NotificationKind.PARTIALLY_AVAILABLE]: "Your Request is Partially Available",
  [NotificationKind.AVAILABLE]: "Your Request is Available",
  [NotificationKind.CANCELLED_BY_DELETION]:
    "Your Request is cancelled because its media was deleted",
  [NotificationKind.NEW_PENDING]: "Someone makes a Request that needs approving",
};
