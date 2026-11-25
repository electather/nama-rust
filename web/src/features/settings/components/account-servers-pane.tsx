import { LinkedServerItems } from "@/features/settings/components/linked-server-items";
import { LinkedUserItems } from "@/features/settings/components/linked-user-items";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountServerLabels } from "@/features/settings/lib/pane-labels";

/** The Servers the Account signed in at the Hub owns or has a User on, and its Linked Users. */
export function AccountServersPane() {
  return (
    <PaneFrame section="account-servers">
      <SettingGroup setting="servers" title={accountServerLabels.servers.label}>
        <LinkedServerItems />
      </SettingGroup>
      <SettingGroup setting="users" title={accountServerLabels.users.label}>
        <LinkedUserItems />
      </SettingGroup>
    </PaneFrame>
  );
}
