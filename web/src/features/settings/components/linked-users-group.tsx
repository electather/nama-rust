import { LinkedUserItems } from "@/features/settings/components/linked-user-items";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountServerLabels } from "@/features/settings/lib/pane-labels";

/** The Account's Linked Users, which load on their own, so Unlink works without the Servers. */
export function LinkedUsersGroup() {
  return (
    <SettingGroup setting="users" title={accountServerLabels.users.label}>
      <SectionBoundary what="your Users">
        <LinkedUserItems />
      </SectionBoundary>
    </SettingGroup>
  );
}
