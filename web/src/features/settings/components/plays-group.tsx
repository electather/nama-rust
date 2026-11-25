import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { ViewingEventItems } from "@/features/settings/components/viewing-event-items";
import { accountHistoryLabels } from "@/features/settings/lib/pane-labels";

/** What the Account's Linked Users played, which loads on its own, so pooling works without it. */
export function PlaysGroup() {
  return (
    <SettingGroup setting="plays" title={accountHistoryLabels.plays.label}>
      <p className="pt-1 pb-1.5 text-sm text-pretty text-muted-foreground">
        Latest first, kept as long as your Account. Titles go by their External ID, such as
        tmdb-603: the Hub keeps no names.
      </p>
      <SectionBoundary what="what your Linked Users played" rows={3}>
        <ViewingEventItems />
      </SectionBoundary>
    </SettingGroup>
  );
}
