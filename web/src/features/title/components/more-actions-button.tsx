import { Ellipsis } from "lucide-react";
import type { ComponentProps } from "react";

import { Button } from "@/shared/ui/button";

/**
 * The "…" that opens a menu of what else may be done, named `label`: round beside a title's Play,
 * or `quiet` over a card's art, as Continue Watching's.
 */
export function MoreActionsButton({
  label,
  quiet = false,
  ...props
}: ComponentProps<typeof Button> & { label: string; quiet?: boolean | undefined }) {
  return (
    <Button variant={quiet ? "overlay" : "secondary"} size="round" aria-label={label} {...props}>
      <Ellipsis aria-hidden />
    </Button>
  );
}
