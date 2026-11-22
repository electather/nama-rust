import { NumberInput } from "nama-ui/components/number-field";
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
    <div className="w-36">
      <NumberInput
        aria-label={label}
        placeholder={placeholder}
        value={typed}
        onValueChange={(text) => {
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
