import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PlaysGroup } from "@/features/settings/components/plays-group";
import { PoolingRow } from "@/features/settings/components/pooling-row";
import { SectionBoundary } from "@/features/settings/components/section-boundary";

/**
 * Whether the Movies the Linked Users of the Account signed in at the Hub finish are pooled, and
 * what those Users played, with deleting and exporting it.
 */
export function AccountHistoryPane() {
  return (
    <PaneFrame section="account-history">
      <SectionBoundary what="whether your plays are counted">
        <PoolingRow />
      </SectionBoundary>
      <PlaysGroup />
    </PaneFrame>
  );
}
