import { type ComponentProps, useState } from "react";

import { SettingRow } from "@/shared/components/setting-row";
import { Input } from "@/shared/ui/input";

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
  input: ComponentProps<typeof Input>;
  onSave: (typed: string) => void;
}) {
  const [typed, setTyped] = useState(saved);
  // What was last sent to be saved, so leaving the field again doesn't send it twice.
  const [sent, setSent] = useState(saved);
  const [left, setLeft] = useState(false);
  const problem = check?.(typed);
  const shown = left ? (problem ?? error) : undefined;

  /** Saves what's typed, unless `check` finds it wrong or it's `already` the value to save. */
  function commit(already: string) {
    const value = typed.trim();
    setLeft(true);
    if (problem === undefined && value !== already) {
      setSent(value);
      onSave(value);
    }
  }

  return (
    <SettingRow label={label} hint={hint} error={shown}>
      {(control) => (
        <span className="flex w-40 md:w-60">
          <Input
            {...input}
            {...control}
            aria-invalid={shown !== undefined}
            value={typed}
            onChange={(event) => {
              setTyped(event.target.value);
            }}
            onBlur={() => {
              commit(sent);
            }}
            onKeyDown={(event) => {
              // Enter sends whatever the Server doesn't have, so it also tries a refused one again.
              if (event.key === "Enter") {
                commit(saved);
              }
            }}
          />
        </span>
      )}
    </SettingRow>
  );
}
