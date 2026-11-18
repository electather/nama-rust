import { type ReactNode, useState } from "react";

import { NameThisDevice } from "@/features/auth/components/name-this-device";
import { WaitForApproval } from "@/features/auth/components/wait-for-approval";
import { WhereToEnterIt } from "@/features/auth/components/where-to-enter-it";
import { useStartSignInCode } from "@/features/auth/data/use-start-sign-in-code";
import { startErrors } from "@/features/auth/lib/sign-in-errors";
import { type StartSignInCodeResponse, SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { SignInPage } from "@/shared/components/sign-in-layout/sign-in-page";
import { Button } from "@/shared/ui/button";

/**
 * Signing this browser in without a password: it shows a Sign-in Code, and a User signed in on
 * another Device approves it at `/code`.
 */
export function CodeSignIn({
  onSignedIn,
  footer,
}: {
  /** Called once, when the code is approved and this browser holds its session. */
  onSignedIn: () => Promise<void>;
  /** Another way to sign in instead. */
  footer: ReactNode;
}) {
  // Kept while getting a new code, so the expired one stays up until the new one replaces it.
  const [shown, setShown] = useState<StartSignInCodeResponse>();
  const [deviceName, setDeviceName] = useState("");
  const start = useStartSignInCode(setShown);
  const errors = start.error ? startErrors(start.error) : {};
  const getCode = (name: string) => {
    setDeviceName(name);
    start.mutate({ deviceName: name, delivery: SessionDelivery.COOKIE });
  };

  if (!shown) {
    return (
      <NameThisDevice
        onSubmit={getCode}
        pending={start.isPending}
        errors={errors}
        footer={footer}
      />
    );
  }
  return (
    <SignInPage heading="Sign in with a code" intro={<WhereToEnterIt />} footer={footer}>
      <WaitForApproval key={shown.handle} shown={shown} onSignedIn={onSignedIn}>
        {/* The Device name field is gone by now, so its refusal shows here. */}
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {errors.form ?? errors.deviceName}
        </p>
        <Button
          disabled={start.isPending}
          onClick={() => {
            getCode(deviceName);
          }}
        >
          Get a new code
        </Button>
      </WaitForApproval>
    </SignInPage>
  );
}
