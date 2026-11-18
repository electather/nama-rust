import { PreferenceCheckbox } from "@/features/settings/components/preference-checkbox";
import { useNotificationPreferences } from "@/features/settings/data/use-notification-preferences";

/**
 * The kinds of Request notification the signed-in User gets on every app they connected, each a
 * checkbox that saves as it's ticked.
 */
export function NotificationPreferences() {
  const { data } = useNotificationPreferences();
  if (!data) {
    return null;
  }
  return (
    <section aria-labelledby="notifications" className="mt-12 max-w-2xl">
      <h2 id="notifications" className="font-medium">
        Notifications
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What Nama messages you about, in every app you connect.
      </p>
      <ul className="mt-4 flex flex-col gap-3">
        {data.preferences.map((preference) => (
          <PreferenceCheckbox key={preference.kind} preference={preference} />
        ))}
      </ul>
    </section>
  );
}
