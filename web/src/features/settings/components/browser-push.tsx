import { ConnectError } from "@connectrpc/connect";

import { SettingGroup } from "@/features/settings/components/setting-group";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { useBrowserPush } from "@/features/settings/data/use-browser-push";
import { useSetBrowserPush } from "@/features/settings/data/use-set-browser-push";
import { browserPushText } from "@/features/settings/lib/notification-labels";
import { notificationsBlocked, webPushMissing } from "@/features/settings/lib/web-push";
import { refusal } from "@/shared/lib/forms";

/**
 * "Notify this browser": the switch that has the Server push the User's notifications to this
 * browser, saved as it's flipped. Where the page can't have Web Push, only why.
 */
export function BrowserPush() {
  const subscription = useBrowserPush();
  const set = useSetBrowserPush();
  const missing = webPushMissing();
  if (missing !== undefined) {
    return (
      <SettingGroup title="This browser">
        <p
          data-setting="browser-push"
          className="py-3.5 text-xs text-pretty text-muted-foreground last:-mb-3.5"
        >
          {missing}
        </p>
      </SettingGroup>
    );
  }
  const blocked = notificationsBlocked();
  return (
    <SettingGroup title="This browser">
      <SwitchRow
        setting="browser-push"
        label={browserPushText.label}
        hint={
          blocked
            ? "Your browser blocks notifications from Nama. Allow them in its settings for this site, then turn this on."
            : "Shows your notifications here, even while Nama isn’t open."
        }
        // Shows the choice being saved until the browser's subscription is read again.
        checked={set.isPending ? set.variables : subscription !== null}
        disabled={blocked}
        error={failure(set.error)}
        onChange={(on) => {
          set.mutate(on);
        }}
      />
    </SettingGroup>
  );
}

/**
 * What went wrong flipping the switch, if anything. Only an RPC's error is the Server's: `refusal`
 * would read the browser's own, such as the `AbortError` of a push service it can't reach, as a
 * cancel.
 */
function failure(error: Error | null) {
  if (error instanceof ConnectError) {
    return refusal(error, "change that", {}).form;
  }
  return error === null
    ? undefined
    : "This browser couldn’t change its notifications. Try again in a moment.";
}
