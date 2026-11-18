import { ChangeButton } from "@/features/requests/components/change-button";
import { useWithdrawRequest } from "@/features/requests/data/use-withdraw-request";
import { type Target, changeRefusal } from "@/features/requests/lib/request-target";

/** Withdrawing can be undone by asking again, so it doesn't ask first. */
export function Withdraw({ id, ...target }: Target) {
  const withdraw = useWithdrawRequest(target.changes);
  return (
    <ChangeButton
      action="Withdraw"
      name={`Withdraw from ${target.label}`}
      change={withdraw}
      id={id}
      refusal={(error) => changeRefusal(error, target, "withdraw from")}
    />
  );
}
