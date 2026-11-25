import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PoolingRow } from "@/features/settings/components/pooling-row";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { ViewingEventItems } from "@/features/settings/components/viewing-event-items";
import { accountHistoryLabels } from "@/features/settings/lib/pane-labels";

/**
 * Whether the Movies the Linked Users of the Account signed in at the Hub finish are pooled, and
 * what those Users played, with deleting and exporting it.
 */
export function AccountHistoryPane() {
  return (
    <PaneFrame section="account-history">
      {/* Last in its own box, so the row gives back its bottom padding before the plays. */}
      <div>
        <PoolingRow />
      </div>
      <SettingGroup setting="plays" title={accountHistoryLabels.plays.label}>
        <p className="pt-1 pb-1.5 text-sm text-pretty text-muted-foreground">
          Latest first, kept as long as your Account. Titles go by their id at a title source, such
          as tmdb-603: the Hub keeps no names.
        </p>
        <ViewingEventItems />
      </SettingGroup>
    </PaneFrame>
  );
}
