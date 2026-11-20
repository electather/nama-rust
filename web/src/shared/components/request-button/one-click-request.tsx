import { Plus } from "lucide-react";

import { Button } from "@/shared/ui/button";

/** The main part of the split Request button, which asks for `what` at the User's Quality. */
export function OneClickRequest({
  what,
  variant,
  disabled,
  onRequest,
}: {
  what: string;
  variant: "default" | "ghost";
  disabled: boolean;
  onRequest: () => void;
}) {
  return (
    <Button
      variant={variant}
      aria-label={`Request ${what}`}
      disabled={disabled}
      onClick={() => {
        onRequest();
      }}
    >
      <Plus aria-hidden className="size-4" />
      Request
    </Button>
  );
}
