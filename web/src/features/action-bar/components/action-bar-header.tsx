import { Film, Search, Tv, X } from "lucide-react";

import { SearchMark } from "@/features/action-bar/components/search-mark";
import { WayOut } from "@/features/action-bar/components/way-out";
import type { Bar } from "@/features/action-bar/hooks/use-action-bar";

const rootPlaceholder = "Search titles, pages, actions…";

// What each frame puts in the field's row: a search of one kind wears that kind as a chip.
const frames = {
  root: { chip: undefined, Icon: Search, placeholder: rootPlaceholder },
  shortcuts: { chip: undefined, Icon: Search, placeholder: rootPlaceholder },
  movie: { chip: "Movie", Icon: Film, placeholder: "Search Movies…" },
  show: { chip: "Show", Icon: Tv, placeholder: "Search Shows…" },
} as const;

/**
 * The action bar's field, with the chip of the kind it's searching and, at its end, the way out.
 */
export function ActionBarHeader({ bar, listId }: { bar: Bar; listId: string }) {
  const { chip, Icon, placeholder } = frames[bar.frame];
  return (
    <div className="flex items-center gap-2.5 border-b p-3.5">
      <SearchMark slow={bar.slow} />
      {chip !== undefined && (
        <button
          type="button"
          aria-label="Clear filter"
          title="Backspace clears the filter"
          onClick={bar.handleBack}
          className="inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full border border-primary/60 bg-accent px-2 text-xs font-medium text-primary transition-colors outline-none hover:bg-accent/70 focus-visible:ring-3 focus-visible:ring-ring/50 motion-reduce:transition-none"
        >
          <Icon aria-hidden strokeWidth={1.5} className="size-3" />
          {chip}
          <X aria-hidden strokeWidth={1.5} className="size-3 opacity-70" />
        </button>
      )}
      <input
        role="combobox"
        aria-expanded
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={bar.active && `${listId}-${bar.active.id}`}
        aria-label="Command menu"
        value={bar.query}
        placeholder={placeholder}
        onChange={(event) => {
          bar.type(event.target.value);
        }}
        onKeyDown={bar.handleFieldKeyDown}
        // A title typed right to left, such as a Persian one, reads right to left.
        dir="auto"
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        // 16px on a phone, where iOS zooms into a field with smaller text.
        className="min-w-0 flex-1 bg-transparent py-1 text-base text-ellipsis text-foreground outline-none placeholder:text-muted-foreground"
      />
      <WayOut bar={bar} />
    </div>
  );
}
