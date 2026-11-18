import type { ReactNode } from "react";

import { ShownCode } from "@/features/auth/components/shown-code";
import { usePollUntilApproved } from "@/features/auth/hooks/use-poll-until-approved";
import type { StartSignInCodeResponse } from "@/gen/nama/v1/sign_in_pb";

/** Shows the code until it's approved, or, once it expires, `newCode` to get another. */
export function WaitForApproval({
  shown,
  onSignedIn,
  children: newCode,
}: {
  shown: StartSignInCodeResponse;
  onSignedIn: () => Promise<void>;
  children: ReactNode;
}) {
  const expired = usePollUntilApproved(shown.handle, onSignedIn);
  return (
    <div className="flex flex-col gap-5">
      <ShownCode code={shown.code} expired={expired} />
      {expired && newCode}
    </div>
  );
}
