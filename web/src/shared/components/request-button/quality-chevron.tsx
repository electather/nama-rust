import { ChevronDown } from "lucide-react";
import type { ComponentProps } from "react";

import { Button } from "@/shared/ui/button";

/**
 * The chevron of the split Request button, which opens the Qualities to Request `what` at. The
 * rest of `props` are the menu trigger's, which Base UI hands it.
 */
export function QualityChevron({
  what,
  variant,
  ...props
}: ComponentProps<typeof Button> & { what: string; variant: "default" | "ghost" }) {
  return (
    <Button {...props} variant={variant} aria-label={`Request ${what} at a Quality`}>
      <ChevronDown aria-hidden className="size-4" />
    </Button>
  );
}
