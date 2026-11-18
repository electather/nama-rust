import { ArrowLeft, Search } from "lucide-react";
import type { KeyboardEvent } from "react";

import { Kbd } from "@/features/action-bar/components/kbd";
import { ScopeChip } from "@/features/action-bar/components/scope-chip";
import type { Frame } from "@/features/action-bar/lib/menu";

const placeholders: Record<Frame, string> = {
  root: "Search Movies, Shows, pages…",
  movies: "Search Movies…",
  shows: "Search Shows…",
  shortcuts: "Search Movies, Shows, pages…",
};

/** The action bar's field, with the kind its search is narrowed to and the key that steps back. */
export function MenuHeader({
  frame,
  query,
  combobox,
  onType,
  onKey,
  onBack,
}: {
  frame: Frame;
  query: string;
  combobox: { listId: string; activeId: string };
  onType: (query: string) => void;
  onKey: (event: KeyboardEvent<HTMLInputElement>) => void;
  onBack: () => void;
}) {
  return (
    <div className="flex items-center gap-2 border-b px-3 py-2.5">
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0 text-muted-foreground" />
      {(frame === "movies" || frame === "shows") && <ScopeChip kind={frame} onClear={onBack} />}
      <input
        role="combobox"
        aria-expanded
        aria-controls={combobox.listId}
        aria-autocomplete="list"
        aria-activedescendant={combobox.activeId}
        aria-label="Command menu"
        value={query}
        placeholder={placeholders[frame]}
        onChange={(event) => {
          onType(event.target.value);
        }}
        onKeyDown={onKey}
        dir="auto"
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        className="min-w-0 flex-1 bg-transparent py-1 text-base text-foreground outline-none placeholder:text-muted-foreground md:text-sm"
      />
      <Kbd bordered>
        {frame === "root" ? "Esc" : <ArrowLeft aria-label="Back" className="size-3" />}
      </Kbd>
    </div>
  );
}
