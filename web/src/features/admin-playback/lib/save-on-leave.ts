import type { FocusEvent, KeyboardEvent } from "react";

/**
 * A text or number field showing `shown`, which saves what's typed once it's left or Enter is
 * pressed in it, never while it's typed, and only if it differs from `shown`.
 */
export function saveOnLeave(shown: string, onSave: (typed: string) => void) {
  const commit = ({ value }: HTMLInputElement) => {
    if (value !== shown) {
      onSave(value);
    }
  };
  return {
    // Uncontrolled, so a refused value stays typed under its refusal while `shown` goes back.
    defaultValue: shown,
    onBlur: (event: FocusEvent<HTMLInputElement>) => {
      commit(event.currentTarget);
    },
    onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        commit(event.currentTarget);
      }
    },
  };
}
