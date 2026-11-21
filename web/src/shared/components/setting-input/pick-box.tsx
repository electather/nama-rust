import type { ReactNode } from "react";

/** One checkbox of a `PickList`, labelled `children`, sending `value` as a `name` form value when ticked. */
export function PickBox({
  name,
  value,
  checked,
  invalid,
  described,
  children,
}: {
  name: string;
  value: string;
  checked: boolean;
  invalid: boolean;
  described: string;
  children: ReactNode;
}) {
  return (
    <label className="flex items-center gap-3 text-sm">
      <input
        type="checkbox"
        name={name}
        value={value}
        defaultChecked={checked}
        aria-invalid={invalid}
        aria-describedby={described}
        className="size-4 accent-foreground"
      />
      {children}
    </label>
  );
}
