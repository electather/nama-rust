import { useState } from "react";

import type { Confirmation } from "@/shared/components/confirm-dialog/confirm-modal";
import { onSubmitted } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/** The form inside a `ConfirmModal`: its words, fields, the Server's refusal and its buttons. */
export function ConfirmForm({
  id,
  close,
  destructive,
  action,
  dismiss = "Cancel",
  title,
  description,
  children,
  onConfirm,
  refusal,
}: Confirmation & { id: string; close: () => void; destructive: boolean }) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();

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
      className="flex flex-col gap-4"
      onSubmit={onSubmitted((typed) => {
        void confirm(typed);
      })}
    >
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
          {dismiss}
        </Button>
        <Button type="submit" variant={destructive ? "destructive" : "default"} disabled={pending}>
          {action}
        </Button>
      </div>
    </form>
  );
}
