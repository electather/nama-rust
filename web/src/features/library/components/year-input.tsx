import { Input } from "nama-ui/components/input";
import { useState } from "react";

/**
 * A year field that changes the listing once it holds a four-digit year, or is cleared, and
 * follows `year` when something else changes it, such as Clear filters.
 */
export function YearInput({
  label,
  placeholder,
  year,
  change,
}: {
  label: string;
  placeholder: string;
  year: number | undefined;
  change: (year?: number) => void;
}) {
  const [typed, setTyped] = useState(year === undefined ? "" : String(year));
  const [heard, setHeard] = useState(year);
  if (year !== heard) {
    setHeard(year);
    setTyped(year === undefined ? "" : String(year));
  }
  return (
    <div className="min-w-0 flex-1">
      <Input
        aria-label={label}
        placeholder={placeholder}
        inputMode="numeric"
        value={typed}
        onChange={({ target: { value: text } }) => {
          setTyped(text);
          if (text === "") {
            change();
          } else if (/^\d{4}$/u.test(text)) {
            change(Number(text));
          }
        }}
      />
    </div>
  );
}
