import { type RefObject, useEffect } from "react";

function outside(event: PointerEvent, dialog: HTMLDialogElement) {
  const box = dialog.getBoundingClientRect();
  return (
    event.clientX < box.left ||
    event.clientX > box.right ||
    event.clientY < box.top ||
    event.clientY > box.bottom
  );
}

/** Closes `dialog` when a press starts and ends outside it, until `signal` aborts. */
function closeOnPressOutside(dialog: HTMLDialogElement, signal: AbortSignal) {
  let pressedOutside = false;
  dialog.addEventListener(
    "pointerdown",
    (event) => {
      pressedOutside = outside(event, dialog);
    },
    { signal },
  );
  dialog.addEventListener(
    "pointerup",
    (event) => {
      // Both ends outside, so a drag that selects text and ends on the backdrop keeps it open.
      if (pressedOutside && outside(event, dialog)) {
        dialog.close();
      }
    },
    { signal },
  );
}

/** Closes `dialog` on a press outside it where `closedby="any"` doesn't yet (Safari). */
export function useLightDismiss(dialog: RefObject<HTMLDialogElement | null>) {
  useEffect(() => {
    const controller = new AbortController();
    if (dialog.current !== null && !("closedBy" in HTMLDialogElement.prototype)) {
      closeOnPressOutside(dialog.current, controller.signal);
    }
    return () => {
      controller.abort();
    };
  }, [dialog]);
}
