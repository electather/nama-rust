import { Button } from "nama-ui/components/button";

import { useResetHomeLayout } from "@/features/settings/data/use-reset-home-layout";
import { refusal } from "@/shared/lib/forms";

/** Drops the signed-in User's own Home layout, so their Home follows the Server's again. */
export function ResetHomeLayout() {
  const reset = useResetHomeLayout();
  return (
    <div className="flex flex-col items-end gap-1">
      <Button
        type="button"
        variant="secondary"
        disabled={reset.isPending}
        onClick={() => {
          reset.mutate({});
        }}
      >
        Reset to default
      </Button>
      {reset.error && (
        <p role="alert" className="text-xs text-destructive">
          {refusal(reset.error, "reset your Home", {}).form}
        </p>
      )}
    </div>
  );
}
