import { Search } from "lucide-react";

import type { SearchState } from "@/features/settings/hooks/use-settings-search";

/**
 * The field that finds settings, a combobox over the results `listId` lists in place of the
 * sections while it has text. Focus stays here while Down and Up pick a result.
 */
export function SettingsSearch({ search, listId }: { search: SearchState; listId: string }) {
  const { typed, picked, results } = search;
  return (
    // DESIGN.md, "Inputs", drawn around the icon and the field together.
    <label className="flex h-11 shrink-0 items-center gap-2 rounded-lg border border-input bg-input/30 px-2.5 text-muted-foreground transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 max-md:mx-4 max-md:mb-2 md:h-7 md:px-2">
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
      <input
        role="combobox"
        aria-label="Search settings"
        aria-expanded={results.length > 0}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={picked && `${listId}-${results.indexOf(picked)}`}
        placeholder="Search settings"
        // Opening Settings starts here, so typing finds a setting at once. Not on a phone, where
        // focus here would open the keyboard over Settings every time; `md` is 48rem.
        // oxlint-disable-next-line jsx-a11y/no-autofocus -- a modal dialog puts focus on the control its design names, which this is
        autoFocus={globalThis.matchMedia("(min-width: 48rem)").matches}
        value={typed}
        onChange={(event) => {
          search.type(event.target.value);
        }}
        onKeyDown={search.handleKeyDown}
        spellCheck={false}
        autoComplete="off"
        // 16px on a phone, where iOS zooms into a field with smaller text.
        className="min-w-0 flex-1 bg-transparent text-base text-foreground outline-none placeholder:text-muted-foreground md:text-sm"
      />
    </label>
  );
}
