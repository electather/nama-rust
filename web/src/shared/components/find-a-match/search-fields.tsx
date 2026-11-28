import { NumberInput } from "nama-ui/components/number-field";
import { useId } from "react";

import { SearchField } from "@/shared/components/search-field";

/** The Title and Year to search for, which Find a Match searches for as they're typed. */
export function SearchFields({
  typed,
  onType,
}: {
  typed: { title: string; year: string };
  onType: (typed: { title: string; year: string }) => void;
}) {
  const titleId = useId();
  const yearId = useId();
  return (
    <div className="flex gap-2.5">
      <div className="min-w-0 flex-1">
        <label htmlFor={titleId} className="sr-only">
          Title
        </label>
        <SearchField
          id={titleId}
          type="search"
          value={typed.title}
          onValueChange={(title) => {
            onType({ ...typed, title });
          }}
        />
      </div>
      <div className="w-36 shrink-0">
        <label htmlFor={yearId} className="sr-only">
          Year
        </label>
        <NumberInput
          id={yearId}
          autoComplete="off"
          placeholder="Any year"
          min={1870}
          max={2100}
          value={typed.year}
          onValueChange={(year) => {
            onType({ ...typed, year });
          }}
        />
      </div>
    </div>
  );
}
