import { useId } from "react";

import { Segmented } from "@/features/settings/components/segmented";
import { type KindFilter, kindOptions } from "@/features/settings/lib/plugin-list";
import { SearchField } from "@/shared/components/search-field";

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
      className="sticky top-0 z-10 flex flex-wrap items-center gap-2 bg-background px-4 pt-4 pb-3 md:px-5 md:pt-2"
    >
      <SearchField
        type="search"
        aria-label="Filter Plugins"
        placeholder="Filter Plugins"
        value={typed}
        onValueChange={onType}
        className="flex-1 basis-50"
      />
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
