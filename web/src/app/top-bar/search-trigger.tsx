import { Search } from "lucide-react";

import { modKey } from "@/shared/lib/mod-key";

/** Opens the action bar, showing its shortcut. Under 640px it's only its icon. */
export function SearchTrigger({ onSearch }: { onSearch: () => void }) {
  return (
    <button
      type="button"
      onClick={onSearch}
      aria-label="Search this Server"
      aria-keyshortcuts="Meta+K Control+K"
      className="inline-flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-md border border-input bg-well text-muted-foreground transition-colors duration-150 ease-out-expo outline-none hover:text-foreground-soft focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background motion-reduce:transition-none sm:w-auto sm:min-w-60 sm:justify-start sm:gap-2.5 sm:ps-2.5 sm:pe-2"
    >
      <Search aria-hidden strokeWidth={1.5} className="size-4" />
      <span className="hidden flex-1 text-start text-sm sm:inline">Search this Server</span>
      <kbd className="hidden rounded-sm border bg-muted px-2 py-0.5 font-sans text-xs text-foreground-soft sm:inline">
        {modKey === "⌘" ? "⌘K" : "Ctrl K"}
      </kbd>
    </button>
  );
}
