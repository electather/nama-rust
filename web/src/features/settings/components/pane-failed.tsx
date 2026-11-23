import { TriangleAlert } from "lucide-react";
import { Button } from "nama-ui/components/button";

/** Says `what` couldn't load, such as "your Devices", with a way to try again. */
export function PaneFailed({ what, onRetry }: { what: string; onRetry: () => void }) {
  return (
    <div className="flex flex-wrap items-center gap-x-3.5 gap-y-2.5">
      <p role="alert" className="flex flex-1 basis-50 items-center gap-1 text-sm">
        <TriangleAlert aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
        Couldn’t load {what}.
      </p>
      <Button variant="secondary" onClick={onRetry}>
        Try again
      </Button>
    </div>
  );
}
