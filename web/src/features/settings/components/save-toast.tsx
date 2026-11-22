import { TriangleAlert, X } from "lucide-react";
import { useRef } from "react";

import { StateLabel } from "@/shared/components/state-label";
import { Button } from "@/shared/ui/button";

/**
 * The toast over the foot of a pane that makes each change as it's asked, there only once the
 * Server has refused one: `says` why, until it's dismissed or the next change is made.
 */
export function SaveToast({
  label = "Not saved",
  says,
  onDismiss,
}: {
  /** What didn't happen, in two words. */
  label?: string;
  /** Nothing while no change was refused, or its refusal is said under the field it's about. */
  says: string | undefined;
  onDismiss: () => void;
}) {
  // What had focus before Dismiss took it, to hand it back: Dismiss goes with the toast.
  const before = useRef<HTMLElement>(null);
  if (says === undefined) {
    return null;
  }
  return (
    <div className="sticky bottom-4 z-10 mx-4 mt-auto mb-4 flex items-center gap-2.5 rounded-lg border bg-popover py-1 ps-3.5 pe-1 text-popover-foreground shadow-md md:mx-7.5 md:max-w-lg md:self-end">
      <StateLabel tone="destructive" label={label} Icon={TriangleAlert} />
      {/* Only the words are announced, not the button's name. */}
      <p role="alert" className="min-w-0 flex-1 py-1.5 text-sm text-pretty">
        {says}
      </p>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Dismiss"
        onFocus={(event) => {
          before.current = event.relatedTarget instanceof HTMLElement ? event.relatedTarget : null;
        }}
        onClick={() => {
          before.current?.focus();
          onDismiss();
        }}
      >
        <X aria-hidden />
      </Button>
    </div>
  );
}
