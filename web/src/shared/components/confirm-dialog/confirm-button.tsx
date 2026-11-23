import type { LucideIcon } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { IconAction } from "@/shared/components/icon-action";

/** What opens a confirmation: its `action` in words, or only its `Icon`, the words in a tooltip. */
export function ConfirmButton({
  action,
  name,
  Icon,
  onClick,
}: {
  action: string;
  name: string;
  Icon: LucideIcon | undefined;
  onClick: () => void;
}) {
  if (Icon !== undefined) {
    return (
      <IconAction
        label={action}
        Icon={Icon}
        aria-label={name}
        aria-haspopup="dialog"
        onClick={onClick}
      />
    );
  }
  return (
    <Button variant="ghost" aria-label={name} aria-haspopup="dialog" onClick={onClick}>
      {action}
    </Button>
  );
}
