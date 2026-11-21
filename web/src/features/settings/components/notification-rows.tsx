import { NotificationRow } from "@/features/settings/components/notification-row";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { useNotificationPreferences } from "@/features/settings/data/use-notification-preferences";

/**
 * The kinds of Request notification the signed-in User gets in every app they connected, each a
 * switch that saves as it's flipped.
 */
export function NotificationRows() {
  const preferences = useNotificationPreferences();
  return (
    <SettingGroup title="What you’re messaged about">
      {preferences.map((preference) => (
        <NotificationRow key={preference.kind} preference={preference} />
      ))}
    </SettingGroup>
  );
}
