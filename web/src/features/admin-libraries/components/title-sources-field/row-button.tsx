import type { ReactNode } from "react";

import { Button } from "@/shared/ui/button";

/** An icon button on a title source's row, named by `name`. */
export function RowButton({
  name,
  disabled,
  onClick,
  children,
}: {
  name: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={name}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
