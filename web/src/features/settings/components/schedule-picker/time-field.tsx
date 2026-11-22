import { useId } from "react";

import { PICKER_FIELD } from "@/features/settings/lib/picker-field";

/** The time of day a schedule runs at, on the Server's clock, in the browser's own time field. */
export function TimeField({
  value,
  onChange,
}: {
  value: string;
  onChange: (time: string) => void;
}) {
  const id = useId();
  return (
    <span className="flex items-center gap-2 text-sm">
      <label htmlFor={id}>at</label>
      <input
        id={id}
        type="time"
        step={60}
        required
        value={value}
        onChange={(event) => {
          // A time field reads "" while it's half typed, which isn't a time to keep.
          if (event.target.value !== "") {
            onChange(event.target.value);
          }
        }}
        className={`${PICKER_FIELD} w-28`}
      />
    </span>
  );
}
