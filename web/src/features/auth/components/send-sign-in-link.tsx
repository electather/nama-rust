import { Link } from "@tanstack/react-router";

import { EmailField } from "@/features/auth/components/email-field";
import { useSendSignInLink } from "@/features/auth/data/use-send-sign-in-link";
import { accountErrors } from "@/features/auth/lib/account-errors";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { formText } from "@/shared/lib/forms";
import { textLink } from "@/shared/lib/text-link-classes";

/** Asking the Hub to email a Sign-in Link, which goes on to `redirect` once used. */
export function SendSignInLink({ redirect }: { redirect: string | undefined }) {
  const send = useSendSignInLink();
  const errors = send.error ? accountErrors(send.error) : {};
  return (
    <SignInLayout
      heading="Email a Sign-in Link"
      action={send.isPending ? "Sending…" : "Email me a Sign-in Link"}
      pending={send.isPending}
      errors={errors}
      onSubmit={(form) => {
        send.mutate({ email: formText(form, "email"), continueTo: redirect ?? "/" });
      }}
      footer={
        <p className="text-center text-sm text-muted-foreground">
          <Link to="/sign-in" search={{ redirect }} className={textLink}>
            Sign in with a password
          </Link>
        </p>
      }
    >
      <EmailField
        error={errors.email}
        hint="It signs you in once, within 15 minutes, without a password."
      />
    </SignInLayout>
  );
}
