import { useSetNotificationPreference } from "@/features/settings/data/use-set-notification-preference";
import { notificationLabels } from "@/features/settings/lib/notification-labels";
import type { NotificationPreference } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/shared/lib/forms";

/** One kind of notification's checkbox, saved as it's ticked. */
export function PreferenceCheckbox({
  preference: { kind, on },
}: {
  preference: NotificationPreference;
}) {
  const set = useSetNotificationPreference();
  // Shows the choice being saved until the list comes back with it.
  const checked = set.isPending ? set.variables.on : on;
  return (
    <li>
      <label className="flex items-start gap-3 text-sm">
        <input
          type="checkbox"
          checked={checked === true}
          onChange={(event) => {
            set.mutate({ kind, on: event.target.checked });
          }}
          className="mt-0.5 size-4 shrink-0 accent-foreground"
        />
        {notificationLabels[kind]}
      </label>
      {set.error && (
        <p role="alert" className="mt-1 text-sm text-destructive">
          {refusal(set.error, "save that", {}).form}
        </p>
      )}
    </li>
  );
}
