import { Plus } from "lucide-react";

import { Button } from "@/shared/ui/button";

/** The form's submit button, which Requests what's picked of the Show titled `title`. */
export function SeasonRequestButton({
  title,
  busy,
  hasPopup,
}: {
  title: string;
  busy: boolean;
  hasPopup: false | "dialog";
}) {
  return (
    <div className="mt-6">
      <Button
        type="submit"
        aria-label={`Request Seasons of ${title}`}
        aria-haspopup={hasPopup}
        disabled={busy}
      >
        <Plus aria-hidden className="size-4" />
        Request
      </Button>
    </div>
  );
}
