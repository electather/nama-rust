import { type ReactNode, useId } from "react";

/** What each `PickBox` of a `PickList` takes from it. */
export interface BoxState {
  invalid: boolean;
  described: string;
}

/**
 * A setting picked with checkboxes: its label, the boxes `children` draws with the `BoxState` it's
 * handed, then `hint` and `error`.
 */
export function PickList({
  label,
  hint,
  error,
  children,
}: {
  label: string;
  hint: string;
  error: string | undefined;
  children: (box: BoxState) => ReactNode;
}) {
  const id = useId();
  const described = `${id}-hint ${id}-error`;
  return (
    <fieldset className="flex flex-col gap-2" aria-describedby={described}>
      <legend className="mb-2 text-sm font-medium">{label}</legend>
      {children({ invalid: error !== undefined, described })}
      <p id={`${id}-hint`} className="text-sm text-pretty text-muted-foreground empty:hidden">
        {hint}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}
