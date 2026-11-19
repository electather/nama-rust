import { useNavigate } from "@tanstack/react-router";

import { Approve } from "@/features/auth/components/approve";
import { useApproveSignInCode } from "@/features/auth/data/use-approve-sign-in-code";
import { codeErrors } from "@/features/auth/lib/sign-in-errors";

/**
 * Asks before signing in the Device that showed `code`. Either way it gives way to the next step,
 * so going back doesn't ask again: "done", or back to the code, kept, saying why it was refused.
 */
export function Approval({ code, deviceName }: { code: string; deviceName: string }) {
  const navigate = useNavigate();
  const approve = useApproveSignInCode(
    deviceName,
    async () => {
      await navigate({ to: "/code/done", state: { signedInDevice: deviceName }, replace: true });
    },
    async (refusal) => {
      const refusedCode = { code, errors: codeErrors(refusal) };
      await navigate({ to: "/code", state: { refusedCode }, replace: true });
    },
  );
  return (
    <Approve
      deviceName={deviceName}
      pending={approve.isPending}
      onApprove={() => {
        approve.mutate({ code });
      }}
      onCancel={() => {
        void navigate({ to: "/code", replace: true });
      }}
    />
  );
}
