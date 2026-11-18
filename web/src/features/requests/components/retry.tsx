import { ChangeButton } from "@/features/requests/components/change-button";
import { useRetryRequest } from "@/features/requests/data/use-retry-request";
import { type Target, changeRefusal } from "@/features/requests/lib/request-target";

/** Hands a Failed Request to its Fulfiller again. */
export function Retry({ id, ...target }: Target) {
  const retry = useRetryRequest();
  return (
    <ChangeButton
      action="Retry"
      name={`Retry ${target.label}`}
      change={retry}
      id={id}
      refusal={(error) => changeRefusal(error, target, "retry")}
      quality={target.quality}
    />
  );
}
