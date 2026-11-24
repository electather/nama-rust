import { ServerSignInRefused } from "@/features/auth/components/server-sign-in-refused";
import { serverSignInErrors } from "@/features/auth/lib/hub-errors";

/** A sign-in to a Server the Hub couldn't look up, such as one that expired, and why. */
export function ServerSignInFailed({ error }: { error: unknown }) {
  return <ServerSignInRefused heading="Sign in to a Server" errors={serverSignInErrors(error)} />;
}
