import { Link, useSearch } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import { useModalContainer } from "@/shared/hooks/use-modal-container";

const LABEL = "New Library";

/** New Library, as a plus beside Settings' close, while the Libraries are listed. */
export function NewLibraryLink() {
  const opened = useSearch({ strict: false, select: ({ library }) => library !== undefined });
  const [trigger, container] = useModalContainer();
  if (opened) {
    return null;
  }
  const link = (
    <Link
      ref={trigger}
      to="."
      search={(rest) => ({ ...rest, library: "new" })}
      aria-label={LABEL}
      className={buttonVariants({ variant: "ghost", size: "icon-sm" })}
    >
      <Plus aria-hidden strokeWidth={1.5} />
    </Link>
  );
  return (
    <Tooltip>
      <TooltipTrigger render={link} />
      {/* The link's own name says it, so the tooltip is only seen. */}
      <TooltipContent container={container} aria-hidden>
        {LABEL}
      </TooltipContent>
    </Tooltip>
  );
}
