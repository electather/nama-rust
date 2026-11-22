import { type FocusEvent, type KeyboardEvent, type SyntheticEvent, useRef } from "react";

import { SAVES_ON_CHANGE } from "@/shared/lib/forms";

/** Whether `field` is typed in, so that half of what it will hold isn't for saving. */
function isTyped(field: EventTarget) {
  const typed = field instanceof HTMLInputElement && !["checkbox", "radio"].includes(field.type);
  return typed || field instanceof HTMLTextAreaElement;
}

/** Whether `field` only narrows a combobox's list: picking from the list saves (`ChipsField`). */
function narrows(field: EventTarget) {
  return field instanceof HTMLElement && field.getAttribute("role") === "combobox";
}

/** Submits `form`, unless the browser finds a field wrong: then it says which, and why. */
function submit(form: HTMLFormElement) {
  if (form.reportValidity()) {
    form.requestSubmit();
  }
}

/**
 * What makes a native form save itself, to spread on its `<form>`: it submits when a switch, a
 * checkbox or a dropdown changes, and for a field that's typed in, once that's left or Enter is
 * pressed in it. Its `onSubmit` does the saving, as for a form with a button.
 */
export function useAutoSubmit() {
  // Whether a typed field holds something not sent yet.
  const unsent = useRef(false);
  return {
    // So a field the browser doesn't report changes of saves it too (`changed`).
    [SAVES_ON_CHANGE]: "",
    onChange: ({ target, currentTarget }: SyntheticEvent<HTMLFormElement>) => {
      if (narrows(target)) {
        return;
      }
      // React's change comes with every key, so a typed field waits to be left.
      if (isTyped(target)) {
        unsent.current = true;
      } else {
        submit(currentTarget);
      }
    },
    onBlur: ({ currentTarget }: FocusEvent<HTMLFormElement>) => {
      if (unsent.current) {
        unsent.current = false;
        submit(currentTarget);
      }
    },
    onKeyDown: (event: KeyboardEvent<HTMLFormElement>) => {
      if (event.key === "Enter" && isTyped(event.target) && !narrows(event.target)) {
        // Without a submit button, Enter alone sends nothing.
        event.preventDefault();
        unsent.current = false;
        submit(event.currentTarget);
      }
    },
  };
}
