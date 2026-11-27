import { Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { openedId, detailSearch } from "@/shared/lib/settings-address";

const LABEL = "New Library";

/** New Library, as a plus beside Settings' close, while the Libraries are listed. */
export function NewLibraryLink() {
  const opened = openedId(useSettingsAddress().detail, "library") !== undefined;
  const [trigger, container] = useModalContainer();
  if (opened) {
    return null;
  }
  const link = (
    <Link
      ref={trigger}
      to="."
      search={(rest) => ({ ...rest, ...detailSearch({ kind: "library", id: "new" }) })}
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
