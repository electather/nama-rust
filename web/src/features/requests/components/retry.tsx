import { RotateCw } from "lucide-react";

import { ChangeButton } from "@/features/requests/components/change-button";
import { type Target, changeRefusal } from "@/features/requests/lib/request-target";
import { useRetryRequest } from "@/shared/hooks/use-retry-request";

/** Hands a Failed Request to its Fulfiller again. */
export function Retry({ id, ...target }: Target) {
  const retry = useRetryRequest();
  return (
    <ChangeButton
      action="Retry"
      Icon={RotateCw}
      name={`Retry ${target.label}`}
      change={retry}
      id={id}
      refusal={(error) => changeRefusal(error, target, "retry")}
      quality={target.quality}
    />
  );
}
