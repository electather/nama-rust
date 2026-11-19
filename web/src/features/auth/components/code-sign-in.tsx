import { NameThisDevice } from "@/features/auth/components/name-this-device";
import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { useStartSignInCode } from "@/features/auth/data/use-start-sign-in-code";
import { startErrors } from "@/features/auth/lib/sign-in-errors";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";

/**
 * Signing this browser in without a password: it names this Device and gets a Sign-in Code, which
 * a User signed in on another Device approves at `/code`.
 */
export function CodeSignIn({ redirect }: { redirect: string | undefined }) {
  const start = useStartSignInCode(redirect, false);
  return (
    <NameThisDevice
      onSubmit={(deviceName) => {
        start.mutate({ deviceName, delivery: SessionDelivery.COOKIE });
      }}
      pending={start.isPending}
      errors={start.error ? startErrors(start.error) : {}}
      footer={<SwitchSignInWay to="/sign-in" redirect={redirect} label="Sign in with a password" />}
    />
  );
}
