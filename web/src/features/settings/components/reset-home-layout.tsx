import { RotateCcw } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import { useResetHomeLayout } from "@/features/settings/data/use-reset-home-layout";
import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { refusal } from "@/shared/lib/forms";

const LABEL = "Reset to default";

/**
 * Drops the signed-in User's own Home layout, so their Home follows the Server's again: an icon in
 * the header over the Home pane, named on hover and focus, with why the Server refused it, if it
 * did, before it.
 */
export function ResetHomeLayout() {
  const reset = useResetHomeLayout();
  const [trigger, container] = useModalContainer();
  const button = (
    <Button
      ref={trigger}
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={LABEL}
      disabled={reset.isPending}
      onClick={() => {
        reset.mutate({});
      }}
    >
      <RotateCcw aria-hidden strokeWidth={1.5} />
    </Button>
  );
  return (
    <div className="flex min-w-0 items-center gap-2">
      {reset.error && (
        <p role="alert" className="line-clamp-2 min-w-0 text-xs text-destructive">
          {refusal(reset.error, "reset your Home", {}).form}
        </p>
      )}
      <Tooltip>
        <TooltipTrigger render={button} />
        {/* The button's own name says it, so the tooltip is only seen. */}
        <TooltipContent container={container} aria-hidden>
          {LABEL}
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
