import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { useHubAccount } from "@/shared/hooks/use-hub-account";

/** The way back to a password, left out for a signed-in Account, since `/sign-in` bounces back for one. */
export function PasswordLink({ redirect }: { redirect: string | undefined }) {
  if (useHubAccount() !== null) {
    return null;
  }
  return <SwitchSignInWay to="/sign-in" redirect={redirect} label="Sign in with a password" />;
}
