import { useId, useState } from "react";

import { PICKER_FIELD } from "@/features/settings/lib/picker-field";

/**
 * Cron typed as it is, kept once the field is left or Enter is pressed, never while it's typed,
 * so it isn't called wrong half way through.
 */
export function CustomField({
  value,
  invalid,
  onChange,
}: {
  value: string;
  invalid: boolean;
  onChange: (cron: string) => void;
}) {
  const id = useId();
  const [typed, setTyped] = useState(value);
  const field = (
    <input
      id={id}
      value={typed}
      aria-invalid={invalid}
      aria-describedby={`${id}-hint`}
      autoComplete="off"
      spellCheck={false}
      className={`${PICKER_FIELD} w-44`}
      onChange={(event) => {
        setTyped(event.target.value);
      }}
      onBlur={() => {
        onChange(typed);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          onChange(typed);
        }
      }}
    />
  );
  const labelled = (
    <span className="flex items-center gap-2">
      <label htmlFor={id}>Schedule</label>
      {field}
    </span>
  );
  return (
    <div className="flex flex-col gap-1.5 text-sm">
      {labelled}
      <p id={`${id}-hint`} className="max-w-prose text-xs text-pretty text-muted-foreground">
        Five parts with spaces between: minute, hour, day of the month, month and day of the week.
        For example, 30 2 * * 1-5 runs at 02:30 on weekdays.
      </p>
    </div>
  );
}
