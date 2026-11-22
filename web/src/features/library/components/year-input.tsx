import { Input } from "nama-ui/components/input";
import { useState } from "react";

/** A year field that changes the listing once it holds a four-digit year, or is cleared. */
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
  return (
    <div className="w-24">
      <Input
        type="number"
        inputMode="numeric"
        aria-label={label}
        placeholder={placeholder}
        value={typed}
        onChange={(event) => {
          const text = event.target.value;
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
