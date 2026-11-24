import { Modal } from "nama-ui/components/modal";
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
  onCancel,
  ...confirmation
}: Confirmation & {
  ref: RefObject<HTMLDialogElement | null>;
  openings: number;
  /** Whether confirming can't be undone, which the confirm button shows. */
  destructive?: boolean;
  /** Called whenever it closes: confirmed, cancelled or dismissed with Escape. */
  onClose?: () => void;
  /** Called as soon as Cancel or ✕ is pressed, without waiting for the `close` event. */
  onCancel?: () => void;
}) {
  const id = useId();
  return (
    <Modal ref={ref} id={id} onClose={onClose}>
      <ConfirmForm
        key={openings}
        id={id}
        close={() => ref.current?.close()}
        cancel={() => {
          onCancel?.();
          ref.current?.close();
        }}
        destructive={destructive}
        {...confirmation}
      />
    </Modal>
  );
}
