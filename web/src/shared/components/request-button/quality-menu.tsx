import { ChevronDown } from "lucide-react";

import { QualityChoices } from "@/shared/components/request-button/quality-choices";
import { useQualities } from "@/shared/hooks/use-qualities";
import { Button } from "@/shared/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/shared/ui/dropdown-menu";

/**
 * The chevron of the split Request button, whose menu Requests `what` at the Quality picked from
 * those of the Fulfiller for `kind`. Nothing when there's no choice.
 */
export function QualityMenu({
  what,
  variant,
  disabled,
  kind,
  onPick,
}: {
  what: string;
  variant: "default" | "ghost";
  disabled: boolean;
  kind: "movie" | "show";
  onPick: (quality: bigint) => void;
}) {
  const { qualities, personalDefault } = useQualities(kind);
  if (qualities.length < 2) {
    return null;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant={variant}
            aria-label={`Request ${what} at a Quality`}
            disabled={disabled}
          />
        }
      >
        <ChevronDown aria-hidden className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <QualityChoices qualities={qualities} personalDefault={personalDefault} onPick={onPick} />
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
