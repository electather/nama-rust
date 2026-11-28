import type { SearchState } from "@/features/settings/hooks/use-settings-search";
import { SearchField } from "@/shared/components/search-field";

/**
 * The field that finds settings, a combobox over the results `listId` lists in place of the
 * sections while it has text. Focus stays here while Down and Up pick a result.
 */
export function SettingsSearch({ search, listId }: { search: SearchState; listId: string }) {
  const { typed, picked, results } = search;
  // The field's own input is the combobox.
  const combobox = {
    role: "combobox",
    "aria-expanded": results.length > 0,
    "aria-controls": listId,
    "aria-autocomplete": "list",
    "aria-activedescendant": picked && `${listId}-${results.indexOf(picked)}`,
  } as const;
  return (
    <SearchField
      {...combobox}
      aria-label="Search settings"
      placeholder="Search settings"
      value={typed}
      onValueChange={(words) => {
        search.type(words);
      }}
      onKeyDown={search.handleKeyDown}
      className="shrink-0 max-md:mx-4 max-md:mb-2"
    />
  );
}
