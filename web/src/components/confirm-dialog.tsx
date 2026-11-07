import { type ReactNode, useId, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { onSubmitted } from "@/lib/forms";

export interface Confirmation {
  /** What the button and the dialog's confirm button say, such as "Revoke". */
  action: string;
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
 * A button that asks in a dialog before doing something that can't be undone. The dialog stays
 * open, saying why, if the Server refuses, and closes once it's done.
 */
export function ConfirmDialog({
  name,
  ...confirmation
}: Confirmation & {
  /** The button's accessible name, which says what it acts on, such as "Revoke Laptop". */
  name: string;
}) {
  // ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself.
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  // Each opening starts afresh, without the last refusal or what was typed.
  const [openings, setOpenings] = useState(0);
  return (
    <>
      <Button
        variant="ghost"
        aria-label={name}
        aria-haspopup="dialog"
        onClick={() => {
          setOpenings(openings + 1);
          dialog.current?.showModal();
        }}
      >
        {confirmation.action}
      </Button>
      <dialog
        ref={dialog}
        role="alertdialog"
        aria-labelledby={`${id}-title`}
        aria-describedby={`${id}-description`}
        // Floats above the page, so it alone gets a shadow (DESIGN.md, "Elevation").
        className="m-auto w-full max-w-xs rounded-xl border bg-popover p-6 text-popover-foreground shadow-xl backdrop:bg-scrim sm:max-w-sm"
      >
        <ConfirmForm
          key={openings}
          id={id}
          close={() => dialog.current?.close()}
          {...confirmation}
        />
      </dialog>
    </>
  );
}

function ConfirmForm({
  id,
  close,
  action,
  title,
  description,
  children,
  onConfirm,
  refusal,
}: Confirmation & { id: string; close: () => void }) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();

  async function confirm(typed: FormData) {
    setPending(true);
    try {
      await onConfirm(typed);
      close();
    } catch (caught) {
      setError(refusal(caught));
    } finally {
      setPending(false);
    }
  }

  return (
    <form className="flex flex-col gap-4" onSubmit={onSubmitted((typed) => void confirm(typed))}>
      <h2 id={`${id}-title`} className="text-lg font-semibold tracking-tight text-balance">
        {title}
      </h2>
      <p id={`${id}-description`} className="text-sm text-pretty text-muted-foreground">
        {description}
      </p>
      {children}
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button variant="ghost" onClick={close}>
          Cancel
        </Button>
        <Button type="submit" variant="destructive" disabled={pending}>
          {action}
        </Button>
      </div>
    </form>
  );
}
