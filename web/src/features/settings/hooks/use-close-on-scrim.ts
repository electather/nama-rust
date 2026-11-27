import { type RefObject, useEffect } from "react";

import { holdsText } from "@/features/settings/lib/holds-text";
import { phoneWidth } from "@/features/settings/lib/phone-width";

/**
 * Whether `event` is a tap on a phone's scrim above the `sheet`. The sheet's content covers the
 * dialog's own box, so a click on the dialog itself above its top is on the backdrop.
 */
function onScrim(sheet: HTMLDialogElement, event: MouseEvent) {
  const above = event.target === sheet && event.clientY < sheet.getBoundingClientRect().top;
  return above && phoneWidth();
}

/** Whether a text field in focus, or its form, holds text a close would throw away. */
function typing() {
  const focused = document.activeElement;
  return focused !== null && holdsText(focused);
}

/**
 * Calls `close` on a tap on the scrim above Settings' sheet on a phone, as a phone's own sheets close,
 * unless typed text would be lost, as Escape spares it. Not `closedby="any"`: wider, a tap outside
 * leaves Settings open, and Safari lacks `closedby`.
 */
export function useCloseOnScrim(dialog: RefObject<HTMLDialogElement | null>, close: () => void) {
  useEffect(() => {
    const sheet = dialog.current;
    const tap = (event: MouseEvent) => {
      if (sheet !== null && onScrim(sheet, event) && !typing()) {
        close();
      }
    };
    sheet?.addEventListener("click", tap);
    return () => sheet?.removeEventListener("click", tap);
  }, [dialog, close]);
}
