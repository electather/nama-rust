import type { RefObject } from "react";

import { OpenerItem } from "@/features/title/components/opener-item";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import type { Opener } from "@/shared/lib/opener";

/** With Manage Libraries, Fix Match and Unmatch, which open the dialogs `fix` and `unmatch` hold. */
export function MatchItems({
  fix,
  unmatch,
}: {
  fix: RefObject<Opener | null>;
  unmatch: RefObject<Opener | null>;
}) {
  if (!useMay(Permission.MANAGE_LIBRARIES)) {
    return null;
  }
  return (
    <>
      <OpenerItem opener={fix}>Fix Match</OpenerItem>
      <OpenerItem opener={unmatch}>Unmatch</OpenerItem>
    </>
  );
}
