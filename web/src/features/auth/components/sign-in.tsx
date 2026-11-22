import { AccountSignIn } from "@/features/auth/components/account-sign-in";
import { PasswordSignIn } from "@/features/auth/components/password-sign-in";
import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";

/**
 * The sign-in page: a username and password, or else a code, on a page of its own, or an Account
 * through the Hub, on a Server Linked to it.
 */
export function SignIn({ redirect }: { redirect: string | undefined }) {
  return (
    <PasswordSignIn
      onSignedIn={useFinishSignIn(redirect)}
      footer={
        <>
          <SwitchSignInWay to="/sign-in/code" redirect={redirect} label="Sign in with a code" />
          <AccountSignIn redirect={redirect} />
          <p className="text-center text-xs text-balance text-muted-foreground">
            New here? Ask whoever runs this Server to add you.
          </p>
        </>
      }
    />
  );
}
