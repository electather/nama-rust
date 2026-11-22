import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { NotificationKind } from "@/gen/nama/v1/user_pb";
import type { SettingText } from "@/shared/lib/setting-text";

// Every switch is under "What you’re messaged about", so each answers to those words.
const words = "notifications messages notify";

/** What each kind of notification tells the User, as its switch says it, in the Server's order. */
export const notificationLabels: Record<NotificationKind, SettingText> = {
  [NotificationKind.UNSPECIFIED]: { label: "" },
  [NotificationKind.APPROVED]: { label: "Your Request is approved", words },
  [NotificationKind.DECLINED]: { label: "Your Request is declined", words },
  [NotificationKind.FAILED]: { label: "Your Request Failed", words },
  [NotificationKind.PARTIALLY_AVAILABLE]: { label: "Your Request is Partially Available", words },
  [NotificationKind.AVAILABLE]: { label: "Your Request is Available", words },
  [NotificationKind.CANCELLED_BY_DELETION]: {
    label: "Your Request is cancelled because its media was deleted",
    words,
  },
  // The Server offers it only to a User who manages Requests.
  [NotificationKind.NEW_PENDING]: {
    label: "Someone makes a Request that needs approving",
    words,
    needs: Permission.MANAGE_REQUESTS,
  },
};

/** What the switch of the notification `kind` says. */
export function notificationLabel(kind: NotificationKind) {
  return notificationLabels[kind]?.label ?? "";
}
