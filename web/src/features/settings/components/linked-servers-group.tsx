import { LinkedServerItems } from "@/features/settings/components/linked-server-items";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountServerLabels } from "@/features/settings/lib/pane-labels";

/** The Account's Servers, which load on their own, so the Users under them work without them. */
export function LinkedServersGroup() {
  return (
    <SettingGroup setting="servers" title={accountServerLabels.servers.label}>
      <SectionBoundary what="your Servers">
        <LinkedServerItems />
      </SectionBoundary>
    </SettingGroup>
  );
}
