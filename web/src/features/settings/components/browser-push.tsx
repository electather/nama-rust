import { SettingGroup } from "@/features/settings/components/setting-group";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { useBrowserPush } from "@/features/settings/data/use-browser-push";
import { useSetBrowserPush } from "@/features/settings/data/use-set-browser-push";
import { browserPushText } from "@/features/settings/lib/notification-labels";
import { webPushUnavailable } from "@/features/settings/lib/web-push";
import { refusal } from "@/shared/lib/forms";

/**
 * "Notify this browser": the switch that has the Server push the User's notifications to this
 * browser, saved as it's flipped. Where it can't, the switch is off and says why.
 */
export function BrowserPush() {
  const subscription = useBrowserPush();
  const set = useSetBrowserPush();
  const unavailable = webPushUnavailable();
  return (
    <SettingGroup title="This browser">
      <SwitchRow
        setting="browser-push"
        label={browserPushText.label}
        hint={unavailable ?? "Shows your notifications here, even while Nama isn’t open."}
        // Shows the choice being saved until the browser's subscription is read again.
        checked={set.isPending ? set.variables : subscription !== null}
        disabled={unavailable !== undefined}
        error={set.error ? refusal(set.error, "change that", {}).form : undefined}
        onChange={(on) => {
          set.mutate(on);
        }}
      />
    </SettingGroup>
  );
}
