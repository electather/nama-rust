import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import { useOfferedQualities } from "@/features/requests/data/use-offered-qualities";
import type { QualityTarget } from "@/features/requests/lib/quality-target";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

/**
 * Makes a Request manager's `change` to a Request, such as approving it, at another of its
 * Fulfiller's Qualities, changing its Quality in the same step. Shown only when the Fulfiller
 * offers another.
 */
export function QualityMenu({
  action,
  name,
  quality: { kind, id: current },
  change,
  id,
}: {
  /** What the change is called, such as "Approve". */
  action: string;
  /** The accessible name of the change's own button, such as "Approve Heat". */
  name: string;
  quality: QualityTarget;
  change: { mutate: (input: { id: bigint; qualityId: bigint }) => void; isPending: boolean };
  id: bigint;
}) {
  const { data } = useOfferedQualities(kind);
  const [trigger, container] = useModalContainer();
  const others = data?.qualities.filter((quality) => quality.id !== current) ?? [];
  if (others.length === 0) {
    return null;
  }
  const items = others.map((quality) => (
    <DropdownMenuItem
      key={quality.id}
      onClick={() => {
        change.mutate({ id, qualityId: quality.id });
      }}
    >
      {quality.name}
    </DropdownMenuItem>
  ));
  const heading = <DropdownMenuLabel>{action} at</DropdownMenuLabel>;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            ref={trigger}
            variant="ghost"
            size="icon-sm"
            aria-label={`${name} at another Quality`}
            disabled={change.isPending}
          />
        }
      >
        <ChevronDown aria-hidden strokeWidth={1.5} />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuGroup>
          {heading}
          {items}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
