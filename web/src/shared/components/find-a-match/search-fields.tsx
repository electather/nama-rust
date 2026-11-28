import { Input } from "nama-ui/components/input";
import { useId } from "react";

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
      <div className="relative min-w-0 flex-1">
        <label htmlFor={titleId} className="sr-only">
          Title
        </label>
        <Input
          id={titleId}
          type="search"
          autoComplete="off"
          value={typed.title}
          onChange={(event) => {
            onType({ ...typed, title: event.target.value });
          }}
        />
      </div>
      <div className="w-24 shrink-0 md:w-28">
        <label htmlFor={yearId} className="sr-only">
          Year
        </label>
        <Input
          id={yearId}
          inputMode="numeric"
          autoComplete="off"
          placeholder="Any year"
          value={typed.year}
          onChange={(event) => {
            onType({ ...typed, year: event.target.value });
          }}
        />
      </div>
    </div>
  );
}
