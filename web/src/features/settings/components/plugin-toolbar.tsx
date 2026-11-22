import { useId } from "react";

import { FilterField } from "@/features/settings/components/filter-field";
import { Segmented } from "@/features/settings/components/segmented";
import { type KindFilter, kindOptions } from "@/features/settings/lib/plugin-list";

/**
 * What narrows the list of Plugins: the words `typed`, and the `kind` to show. Where there's room
 * it stays at the top of the pane while the list scrolls under it.
 */
export function PluginToolbar({
  typed,
  kind,
  onType,
  onKind,
}: {
  typed: string;
  kind: KindFilter;
  onType: (typed: string) => void;
  onKind: (kind: KindFilter) => void;
}) {
  const kindId = useId();
  return (
    <div
      data-sticks
      className="top-0 z-10 flex flex-wrap items-center gap-2 bg-background px-4 pt-4 pb-3 md:sticky md:px-5 md:pt-2"
    >
      <FilterField label="Filter Plugins" typed={typed} onType={onType} />
      <span id={kindId} className="sr-only">
        Where Plugins come from
      </span>
      <Segmented
        aria-labelledby={kindId}
        name="plugin-kind"
        options={kindOptions}
        value={kind}
        onChange={onKind}
      />
    </div>
  );
}
