import { type ReactNode, useId } from "react";

/** What a {@link Choice} shows and does. */
export interface ChoiceProps {
  /** The form field the checkbox fills, with `value` when checked. */
  name: string;
  value?: number;
  label: string;
  /** What the row says after the label, such as the year a Season aired. */
  detail?: string | undefined;
  /** What the choice covers, on a line of its own under the label. */
  description?: string;
  disabled: boolean;
  onChange?: (checked: boolean) => void;
  /** Its state, at the end of the row. */
  children: ReactNode;
  /** What can be done about its state, such as Retry, after it and outside its description. */
  action?: ReactNode;
}

/**
 * One choice as a checkbox, with its detail and its state at the end of the row. The state
 * describes the checkbox, so a screen reader says why one can't be picked.
 */
export function Choice({
  name,
  value,
  label,
  detail,
  description,
  disabled,
  onChange,
  children,
  action,
}: ChoiceProps) {
  const stateId = useId();
  const descriptionId = useId();
  return (
    <li className="flex min-h-12 flex-wrap items-center gap-x-3 gap-y-1 border-b py-2 last:border-b-0">
      <label className="flex items-center gap-3 font-medium has-disabled:text-muted-foreground">
        <input
          type="checkbox"
          name={name}
          value={value}
          disabled={disabled}
          aria-describedby={description === undefined ? stateId : `${descriptionId} ${stateId}`}
          className="size-4 accent-foreground"
          onChange={
            onChange &&
            ((event) => {
              onChange(event.currentTarget.checked);
            })
          }
        />
        {label}
      </label>
      <span className="text-sm text-muted-foreground">{detail}</span>
      <span id={stateId} className="ms-auto">
        {children}
      </span>
      {action}
      {description !== undefined && (
        // Lined up with the label's text, past the checkbox and the gap after it.
        <p id={descriptionId} className="basis-full ps-7 text-sm text-muted-foreground">
          {description}
        </p>
      )}
    </li>
  );
}
