import { useCancelRequest } from "@/features/requests/data/use-cancel-request";
import { type Target, changeRefusal } from "@/features/requests/lib/request-target";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";

/** Cancels a Request or Followed Show for everyone who asked for it, once confirmed. */
export function Cancel({ id, ...target }: Target) {
  const { title, changes } = target;
  const cancel = useCancelRequest(changes);
  return (
    <ConfirmDialog
      action={`Cancel ${changes.noun}`}
      dismiss="Keep it"
      name={`Cancel ${changes.noun} for ${title}`}
      title={`Cancel the ${changes.noun} for ${title}?`}
      description={changes.cancelling}
      onConfirm={async () => {
        await cancel.mutateAsync({ id });
      }}
      refusal={(error) => changeRefusal(error, target, "cancel")}
    />
  );
}
