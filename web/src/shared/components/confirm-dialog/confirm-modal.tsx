import { type ReactNode, type RefObject, useId } from "react";

import { ConfirmForm } from "@/shared/components/confirm-dialog/confirm-form";

/** What a confirm dialog asks, and what confirming does. */
export interface Confirmation {
  /** What the button and the dialog's confirm button say, such as "Revoke". */
  action: string;
  /** What the button that closes the dialog says, when "Cancel" would read like `action`. */
  dismiss?: string;
  title: string;
  description: string;
  /** Fields the dialog asks for, such as a new password. */
  children?: ReactNode;
  /** Does it, with what was typed in the dialog. Rejects if the Server refuses. */
  onConfirm: (typed: FormData) => Promise<unknown>;
  /** Why the Server refused, as a sentence. */
  refusal: (error: unknown) => string | undefined;
}

/**
 * The dialog of a {@link Confirmation}, opened with `showModal()` on `ref`. Bump `openings` before
 * each opening, so it starts afresh, without the last refusal or what was typed.
 */
export function ConfirmModal({
  ref,
  openings,
  destructive = false,
  onClose,
  ...confirmation
}: Confirmation & {
  ref: RefObject<HTMLDialogElement | null>;
  openings: number;
  /** Whether confirming can't be undone, which the confirm button shows. */
  destructive?: boolean;
  /** Called whenever it closes: confirmed, cancelled or dismissed with Escape. */
  onClose?: () => void;
}) {
  const id = useId();
  return (
    // ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself.
    <dialog
      ref={ref}
      role="alertdialog"
      aria-labelledby={`${id}-title`}
      aria-describedby={`${id}-description`}
      onClose={onClose}
      // DESIGN.md's Dialog. Floats above the page, so it alone gets a shadow ("Elevation"). On a
      // phone the browser's own max-width keeps it clear of the edges. Scrolling inside it, or
      // over its backdrop, doesn't scroll the page behind.
      className="m-auto w-full overscroll-contain rounded-lg border bg-background text-foreground shadow-lg backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim sm:max-w-lg"
    >
      <ConfirmForm
        key={openings}
        id={id}
        close={() => ref.current?.close()}
        destructive={destructive}
        {...confirmation}
      />
    </dialog>
  );
}
