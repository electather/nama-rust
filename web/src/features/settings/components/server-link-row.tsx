import { Button } from "nama-ui/components/button";
import type { ReactNode } from "react";

/** What Linking says, beside the button that changes it. */
export function ServerLinkRow({
  says,
  action,
  doing,
  variant,
  pending,
  onClick,
}: {
  says: ReactNode;
  action: string;
  /** What the button says while its change is under way. */
  doing: string;
  variant: "secondary" | "ghost";
  pending: boolean;
  onClick: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p className="text-sm break-all">{says}</p>
      <Button variant={variant} disabled={pending} onClick={onClick}>
        {pending ? doing : action}
      </Button>
    </div>
  );
}
