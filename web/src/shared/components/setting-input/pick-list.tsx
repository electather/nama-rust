import { type ReactNode, useId } from "react";

/** What each `PickBox` of a `PickList` takes from it. */
export interface BoxState {
  invalid: boolean;
  described: string;
}

/**
 * A setting picked with checkboxes: its label over its `hint`, the boxes `children` draws with the
 * `BoxState` it's handed, then `error`. Spaced and ruled as `SettingRow` is, which it sits among.
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
    // The legend floats so the hairline over the fieldset isn't broken by it.
    <fieldset
      className="flex flex-col gap-2 py-3.5 not-first:border-t"
      aria-describedby={described}
    >
      <legend className="float-start text-sm">{label}</legend>
      <p
        id={`${id}-hint`}
        className="-mt-2 mb-1 text-xs text-pretty text-muted-foreground empty:hidden"
      >
        {hint}
      </p>
      {children({ invalid: error !== undefined, described })}
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-xs text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}
