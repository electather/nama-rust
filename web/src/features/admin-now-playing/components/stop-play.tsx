import { useStopPlay } from "@/features/admin-now-playing/data/use-stop-play";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { FormTextarea } from "@/shared/components/form-field/form-textarea";
import { formText, refusal } from "@/shared/lib/forms";

/** Stops the plan `planId` playing `title` for `viewer`, with a message if the Admin writes one. */
export function StopPlay({
  planId,
  title,
  viewer,
}: {
  planId: bigint;
  title: string;
  viewer: string;
}) {
  const stop = useStopPlay();
  return (
    <ConfirmDialog
      action="Stop"
      name={`Stop ${title} for ${viewer}`}
      title={`Stop ${title}?`}
      description="It stops on their Device at once, and your message, if you write one, goes with it. They may play it again."
      onConfirm={async (typed) => {
        await stop.mutateAsync({ planId, message: formText(typed, "message") });
      }}
      refusal={(error) => refusal(error, "stop it", {}).form}
    >
      {/* The Server's limit (`StopPlay`), so it never refuses a message as too long. */}
      <FormTextarea label="Message (optional)" name="message" maxLength={200} required={false} />
    </ConfirmDialog>
  );
}
