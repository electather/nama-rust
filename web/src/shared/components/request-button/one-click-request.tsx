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
      // As tall as Play, as the primary action at the top of a title's page.
      size={variant === "default" ? "tall" : "default"}
      aria-label={`Request ${what}`}
      disabled={disabled}
      // Not `onClick={onRequest}`: the caller's `onRequest` takes an optional Quality, which the
      // click event would fill.
      onClick={() => {
        onRequest();
      }}
    >
      <Plus aria-hidden className="size-4" />
      Request
    </Button>
  );
}
