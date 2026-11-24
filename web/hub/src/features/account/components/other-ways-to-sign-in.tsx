import { EmailASignInLink } from "@hub/features/account/components/email-a-sign-in-link";
import { PasskeySignIn } from "@hub/features/account/components/passkey-sign-in";
import { usePasskeysWork } from "@hub/features/account/data/passkeys";
import { useSendsSignInLinks } from "@hub/features/account/data/sign-in-links";

/** Under the sign-in form: "Or", then a passkey and a Sign-in Link, each where it works. */
export function OtherWaysToSignIn({ continueTo }: { continueTo: string }) {
  // Unknown hides it: password sign-in works on every Hub, a Sign-in Link only on one with mail.
  const links = useSendsSignInLinks() === true;
  const passkeys = usePasskeysWork();
  if (!links && !passkeys) {
    return null;
  }
  return (
    <>
      <p className="flex items-center gap-3.5 text-sm text-muted-foreground">
        <span aria-hidden className="h-px flex-1 bg-border" />
        Or
        <span aria-hidden className="h-px flex-1 bg-border" />
      </p>
      <PasskeySignIn continueTo={continueTo} />
      {links ? <EmailASignInLink continueTo={continueTo} /> : undefined}
    </>
  );
}
