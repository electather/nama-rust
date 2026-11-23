import type { ReactNode, RefObject } from "react";

/**
 * DESIGN.md's Dialog: a native modal `<dialog>`, opened with `showModal()` on `ref`, that asks
 * before something goes ahead. Its title's id is `${id}-title` and its description's
 * `${id}-description`, which name it to screen readers.
 */
export function Modal({
  ref,
  id,
  onClose,
  children,
}: {
  ref: RefObject<HTMLDialogElement | null>;
  id: string;
  /** Called whenever it closes, Escape included. */
  onClose?: (() => void) | undefined;
  children: ReactNode;
}) {
  return (
    // ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself.
    <dialog
      ref={ref}
      role="alertdialog"
      aria-labelledby={`${id}-title`}
      aria-describedby={`${id}-description`}
      onClose={onClose}
      // Floats above the page, so it alone gets a shadow ("Elevation"). On a phone the browser's
      // own max-width keeps it clear of the edges. Scrolling inside it, or over its backdrop,
      // doesn't scroll the page behind.
      className="m-auto w-full overscroll-contain rounded-lg border bg-background text-foreground shadow-lg backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim sm:max-w-lg"
    >
      {children}
    </dialog>
  );
}
