import { Plus } from "lucide-react";

import { Button } from "@/shared/ui/button";

/** The button that Requests the Movie titled `title`, the screen's primary action if `primary`. */
export function RequestMovieButton({
  title,
  primary,
  hasPopup,
  disabled,
  onRequest,
}: {
  title: string;
  primary: boolean;
  hasPopup: false | "dialog";
  disabled: boolean;
  onRequest: () => void;
}) {
  // A ghost button's icon lines up with the caption above it, its padding hanging outside.
  const [variant, hanging] = primary ? (["default"] as const) : (["ghost", "-ms-4"] as const);
  return (
    <span className={hanging}>
      <Button
        variant={variant}
        aria-label={`Request ${title}`}
        aria-haspopup={hasPopup}
        disabled={disabled}
        onClick={onRequest}
      >
        <Plus aria-hidden className="size-4" />
        Request
      </Button>
    </span>
  );
}
