import { Button } from "nama-ui/components/button";
import { useEffect, useRef } from "react";

/**
 * Asks, in a set secret's field, whether to forget it, as it can't be had back: Forget, or Cancel.
 * Asked in place, not in a dialog, as a dialog's own form can't sit inside the settings' form.
 */
export function ForgetSecret({
  label,
  onForget,
  onCancel,
}: {
  label: string;
  onForget: () => void;
  onCancel: () => void;
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    // The X that asked is gone, so the focus goes to the safe answer.
    cancel.current?.focus();
  }, []);
  return (
    <span className="flex w-full items-center justify-end gap-1 md:w-60">
      {/* The row's label names it, so the question needn't, and fits beside its answers. The button
          says it, for whoever hears it alone. */}
      <span className="me-auto text-sm">Forget it?</span>
      <Button type="button" variant="destructive" aria-label={`Forget ${label}`} onClick={onForget}>
        Forget
      </Button>
      <Button ref={cancel} type="button" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
    </span>
  );
}
