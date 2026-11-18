import { ChangeButton } from "@/features/requests/components/change-button";
import { useApproveRequest } from "@/features/requests/data/use-approve-request";
import { type Target, changeRefusal } from "@/features/requests/lib/request-target";

/** Approves a Pending Request or Followed Show, at another Quality if its Fulfiller offers one. */
export function Approve({ id, ...target }: Target) {
  const approve = useApproveRequest(target.changes);
  return (
    <ChangeButton
      action="Approve"
      name={`Approve ${target.label}`}
      change={approve}
      id={id}
      refusal={(error) => changeRefusal(error, target, "approve")}
      quality={target.quality}
    />
  );
}
