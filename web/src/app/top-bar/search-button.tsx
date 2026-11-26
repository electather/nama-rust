import { Search } from "lucide-react";

import { modKey } from "@/shared/lib/mod-key";

/**
 * Opens the action bar, saying what it searches. Drawn as a search field with its keys, or on a
 * phone as an icon alone.
 */
export function SearchButton({ label, onSearch }: { label: string; onSearch: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-keyshortcuts="Meta+K Control+K"
      onClick={onSearch}
      className="inline-flex size-9 items-center justify-center gap-2.5 rounded-lg border border-input bg-input/30 text-sm text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 sm:w-auto sm:min-w-60 sm:justify-start sm:ps-2.5 sm:pe-2"
    >
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
      <span className="hidden flex-1 text-start sm:block">{label}</span>
      <kbd className="hidden rounded-sm border bg-muted px-2 py-0.5 font-sans text-xs text-foreground sm:block">
        {modKey} K
      </kbd>
    </button>
  );
}
