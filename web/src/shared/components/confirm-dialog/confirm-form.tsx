import { Button } from "nama-ui/components/button";
import { useState } from "react";

import { CloseButton } from "@/shared/components/confirm-dialog/close-button";
import type { Confirmation } from "@/shared/components/confirm-dialog/confirm-modal";
import { AlertText } from "@/shared/components/form-field/alert-text";
import { onSubmitted } from "@/shared/lib/forms";

/** The form inside a `ConfirmModal`: its words, fields, the Server's refusal and its buttons. */
export function ConfirmForm({
  id,
  close,
  onCancel,
  destructive,
  action,
  dismiss = "Cancel",
  title,
  description,
  children,
  onConfirm,
  refusal,
}: Confirmation & {
  id: string;
  close: () => void;
  /** Called as soon as Cancel or ✕ is pressed, before it closes. */
  onCancel?: (() => void) | undefined;
  destructive: boolean;
}) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();

  function cancel() {
    onCancel?.();
    close();
  }

  async function confirm(typed: FormData) {
    setPending(true);
    try {
      await onConfirm(typed);
      close();
    } catch (error_) {
      setError(refusal(error_));
    } finally {
      setPending(false);
    }
  }

  return (
    <form
      className="flex flex-col gap-4 p-6"
      onSubmit={onSubmitted((typed) => {
        void confirm(typed);
      })}
    >
      <div className="flex flex-col gap-2 px-6 text-center sm:ps-0 sm:text-start">
        <h2 id={`${id}-title`} className="text-lg leading-none font-semibold text-balance">
          {title}
        </h2>
        <p
          id={`${id}-description`}
          className="text-sm text-pretty wrap-anywhere text-muted-foreground"
        >
          {description}
        </p>
      </div>
      {children}
      <AlertText error={error} />
      <div className="flex flex-col-reverse gap-2 *:w-full sm:flex-row sm:justify-end sm:*:w-auto">
        <Button variant="outline" disabled={pending} onClick={cancel}>
          {dismiss}
        </Button>
        <Button type="submit" variant={destructive ? "destructive" : "default"} disabled={pending}>
          {action}
        </Button>
      </div>
      <CloseButton disabled={pending} onClose={cancel} />
    </form>
  );
}
