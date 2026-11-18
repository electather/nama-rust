import { useOfferedQualities } from "@/features/requests/data/use-offered-qualities";
import type { QualityTarget } from "@/features/requests/lib/quality-target";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

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
  const others = data?.qualities.filter((quality) => quality.id !== current) ?? [];
  if (others.length === 0) {
    return null;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            aria-label={`${name} at another Quality`}
            disabled={change.isPending}
          />
        }
      >
        {action} at…
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        {others.map((quality) => (
          <DropdownMenuItem
            key={quality.id}
            onClick={() => {
              change.mutate({ id, qualityId: quality.id });
            }}
          >
            {quality.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
