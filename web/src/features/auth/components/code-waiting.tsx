import { NewCodeButton } from "@/features/auth/components/new-code-button";
import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { WaitForApproval } from "@/features/auth/components/wait-for-approval";
import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { useStartSignInCode } from "@/features/auth/data/use-start-sign-in-code";
import type { WaitingCode } from "@/features/auth/lib/history-state";
import { startErrors } from "@/features/auth/lib/sign-in-errors";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";
import { useWarp } from "@/shared/components/sign-in-layout/warp";

/** Shows this browser's Sign-in Code until another Device approves it, then signs it in. */
export function CodeWaiting({
  shown,
  redirect,
}: {
  shown: WaitingCode;
  /** Where signing in goes. */
  redirect: string | undefined;
}) {
  // A new code replaces the expired one, which stays up until it does.
  const start = useStartSignInCode(redirect, true);
  useWarp(start.isPending);
  const errors = start.error ? startErrors(start.error) : {};
  // The Device name field is on the page before, so its refusal shows with the form's.
  const notices = errors.deviceName === undefined ? errors : { form: errors.deviceName };
  return (
    <SignInCard heading="Sign in with a code">
      <WaitForApproval key={shown.handle} shown={shown} onSignedIn={useFinishSignIn(redirect)}>
        <FormNotices errors={notices} />
        <NewCodeButton
          pending={start.isPending}
          onClick={() => {
            start.mutate({ deviceName: shown.deviceName, delivery: SessionDelivery.COOKIE });
          }}
        />
      </WaitForApproval>
      <SwitchSignInWay to="/sign-in" redirect={redirect} label="Sign in with a password" />
    </SignInCard>
  );
}
