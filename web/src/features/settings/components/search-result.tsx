import { MarkedLabel } from "@/features/settings/components/marked-label";
import { sectionTitles } from "@/features/settings/lib/sections";
import type { Findable } from "@/features/settings/lib/setting-search";
import { keepInView } from "@/shared/lib/keep-in-view";

/**
 * One result of Settings' search: its Section's name over the setting's, or a Section's name alone,
 * the letters `typed` matched marked. Focus stays in the field, which takes the keys for every
 * result.
 */
export function SearchResult({
  id,
  result,
  typed,
  picked,
  onOpen,
}: {
  id: string;
  result: Findable;
  typed: string;
  picked: boolean;
  onOpen: (result: Findable) => void;
}) {
  const section = sectionTitles[result.section];
  const isSection = result.key === undefined;
  return (
    // oxlint-disable-next-line jsx-a11y/click-events-have-key-events -- the field takes the keys for every result (`use-settings-search.ts`)
    <div
      ref={picked ? keepInView : undefined}
      id={id}
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- an <option> can't hold a Section's name over a setting's
      role="option"
      aria-selected={picked}
      tabIndex={-1}
      aria-label={isSection ? section : `${result.text.label}, ${section}`}
      onClick={() => {
        onOpen(result);
      }}
      className="group flex min-h-11 cursor-default flex-col justify-center rounded-sm px-2 py-1 text-sm transition-colors select-none hover:bg-muted aria-selected:bg-accent motion-reduce:transition-none md:min-h-8"
    >
      {!isSection && (
        // Quiet ink is too faint on a picked result's fill (DESIGN.md, "Neutral").
        <span className="text-xs text-muted-foreground group-hover:text-foreground group-aria-selected:text-foreground">
          {section}
        </span>
      )}
      <MarkedLabel label={result.text.label} typed={typed} />
    </div>
  );
}
