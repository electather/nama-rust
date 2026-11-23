import type { LucideIcon } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";
import type { ComponentProps } from "react";

import { useModalContainer } from "@/shared/hooks/use-modal-container";

/**
 * A button that shows only its `Icon`, with what it does, `label`, in a tooltip on hover and on
 * focus. Its own `aria-label` names it, saying what it acts on.
 */
export function IconAction({
  label,
  Icon,
  ...props
}: { label: string; Icon: LucideIcon } & ComponentProps<typeof Button>) {
  // The tooltip goes inside a modal dialog it's in, such as Settings, as only that shows over it.
  const [trigger, container] = useModalContainer();
  const button = (
    <Button ref={trigger} type="button" variant="ghost" size="icon-sm" {...props}>
      <Icon aria-hidden strokeWidth={1.5} />
    </Button>
  );
  return (
    <Tooltip>
      <TooltipTrigger render={button} />
      {/* The button's own name says it, so the tooltip is only seen. */}
      <TooltipContent container={container} aria-hidden>
        {label}
      </TooltipContent>
    </Tooltip>
  );
}
