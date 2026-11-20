import { useResetHomeLayout } from "@/features/settings/data/use-reset-home-layout";
import { refusal } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/** Drops the signed-in User's own Home layout, so their Home follows the Server's again. */
export function ResetHomeLayout() {
  const reset = useResetHomeLayout();
  return (
    <div className="mt-4">
      <Button
        type="button"
        variant="outline"
        disabled={reset.isPending}
        onClick={() => {
          reset.mutate({});
        }}
      >
        Reset to default
      </Button>
      {reset.error && (
        <p role="alert" className="mt-2 text-sm text-destructive">
          {refusal(reset.error, "reset your Home", {}).form}
        </p>
      )}
    </div>
  );
}
