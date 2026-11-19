import type { ReactNode } from "react";

import { CodeStatus } from "@/features/auth/components/code-status";
import { ExpiresIn } from "@/features/auth/components/expires-in";
import { ShownCode } from "@/features/auth/components/shown-code";
import { WhereToEnterIt } from "@/features/auth/components/where-to-enter-it";
import { usePollUntilApproved } from "@/features/auth/hooks/use-poll-until-approved";
import type { WaitingCode } from "@/features/auth/lib/history-state";

/** Shows the code until it's approved, or, once it expires, `newCode` to get another. */
export function WaitForApproval({
  shown,
  onSignedIn,
  children: newCode,
}: {
  shown: WaitingCode;
  onSignedIn: () => Promise<void>;
  children: ReactNode;
}) {
  const expired = usePollUntilApproved(shown.handle, onSignedIn);
  const countdown = <ExpiresIn expiresAt={shown.expiresAt} />;
  return (
    <>
      <ShownCode code={shown.code} expired={expired}>
        <CodeStatus expired={expired} expiresIn={countdown} />
      </ShownCode>
      {expired ? newCode : <WhereToEnterIt deviceName={shown.deviceName} />}
    </>
  );
}
