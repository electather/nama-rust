import { SwitchRow } from "@/features/settings/components/switch-row";
import { useSetNotificationPreference } from "@/features/settings/data/use-set-notification-preference";
import { notificationLabel } from "@/features/settings/lib/notification-labels";
import type { NotificationPreference } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/shared/lib/forms";

/** One kind of notification's switch, saved as it's flipped. */
export function NotificationRow({
  preference: { kind, on },
}: {
  preference: NotificationPreference;
}) {
  const set = useSetNotificationPreference();
  return (
    <SwitchRow
      setting={String(kind)}
      label={notificationLabel(kind)}
      // Shows the choice being saved until the list comes back with it.
      checked={set.isPending ? set.variables.on === true : on}
      error={set.error ? refusal(set.error, "save that", {}).form : undefined}
      onChange={(checked) => {
        set.mutate({ kind, on: checked });
      }}
    />
  );
}
