import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SessionItems } from "@/features/settings/components/session-items";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountSessionLabels } from "@/features/settings/lib/pane-labels";

/** Where the Account signed in at the Hub is signed in, and signing out the others. */
export function AccountSessionsPane() {
  return (
    <PaneFrame section="account-sessions">
      <SettingGroup setting="sessions" title={accountSessionLabels.sessions.label}>
        <SessionItems />
      </SettingGroup>
    </PaneFrame>
  );
}
