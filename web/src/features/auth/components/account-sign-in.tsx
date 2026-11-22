import { StartAccountSignIn } from "@/shared/components/account-sign-in/start-account-sign-in";
import { useAccountSignIn } from "@/shared/hooks/use-account-sign-in";

/** Signing in with an Account through the Hub, on a Server Linked to it. */
export function AccountSignIn({ redirect }: { redirect: string | undefined }) {
  return useAccountSignIn() === true ? <StartAccountSignIn redirect={redirect} /> : null;
}
