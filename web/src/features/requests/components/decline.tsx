import { useDeclineRequest } from "@/features/requests/data/use-decline-request";
import { type Target, changeRefusal } from "@/features/requests/lib/request-target";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { FormTextarea } from "@/shared/components/form-field/form-textarea";
import { formText } from "@/shared/lib/forms";

/** Declines a Pending Request or Followed Show, with a reason if its manager gives one. */
export function Decline({ id, ...target }: Target) {
  const { label } = target;
  const decline = useDeclineRequest(target.changes);
  return (
    <ConfirmDialog
      action="Decline"
      name={`Decline ${label}`}
      title={`Decline ${label}?`}
      description={`Its Requesters see that it was declined, and why if you say. Anyone can ask for ${label} again.`}
      onConfirm={async (typed) => {
        await decline.mutateAsync({ id, reason: formText(typed, "reason") });
      }}
      refusal={(error) => changeRefusal(error, target, "decline")}
    >
      <FormTextarea label="Reason (optional)" name="reason" required={false} />
    </ConfirmDialog>
  );
}
