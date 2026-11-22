import type { Input } from "nama-ui/components/input";
import { type ComponentProps, useState } from "react";

import { RowInput } from "@/shared/components/row-input";
import { SettingRow } from "@/shared/components/setting-row";
import { inputSize } from "@/shared/lib/control-size";

/**
 * A setting typed on its row, saved once the field is left or Enter is pressed in it, never while
 * it's typed. What's wrong with it shows then too, so it isn't called wrong while it's still being
 * typed: what `check` finds before the Server is asked, or else the `error` it refused it with.
 */
export function TypedSettingRow({
  label,
  hint,
  saved,
  error,
  check,
  input,
  onSave,
}: {
  label: string;
  hint: string;
  saved: string;
  /** Why the Server refused what was last sent, if it did. */
  error: string | undefined;
  check?: (typed: string) => string | undefined;
  /** The field's own attributes; the row owns its value, its events and whether it's invalid. */
  input: Omit<
    ComponentProps<typeof Input>,
    "value" | "onChange" | "onBlur" | "onKeyDown" | "aria-invalid"
  >;
  onSave: (typed: string) => void;
}) {
  const [typed, setTyped] = useState(saved);
  // What was last sent to be saved, so leaving the field again doesn't send it twice.
  const [sent, setSent] = useState(saved);
  const [left, setLeft] = useState(false);
  const problem = check?.(typed);
  const shown = left ? (problem ?? error) : undefined;

  /**
   * Saves what's typed, or `stepped` to by − and +, unless `check` finds it wrong or it's `already`
   * the value to save.
   */
  function commit(already: string, stepped = typed) {
    const value = stepped.trim();
    setLeft(true);
    if (check?.(value) === undefined && value !== already) {
      setSent(value);
      onSave(value);
    }
  }

  return (
    <SettingRow label={label} hint={hint} error={shown} size={inputSize(input.type)}>
      {(control) => (
        <RowInput
          input={input}
          state={{
            control,
            value: typed,
            invalid: shown !== undefined,
            onType: setTyped,
            onBlur: () => {
              commit(sent);
            },
            onKeyDown: (event) => {
              // Enter sends whatever the Server doesn't have, so it also tries a refused one again.
              if (event.key === "Enter") {
                commit(saved);
              }
            },
            onStep: (stepped) => {
              setTyped(stepped);
              commit(sent, stepped);
            },
          }}
        />
      )}
    </SettingRow>
  );
}
