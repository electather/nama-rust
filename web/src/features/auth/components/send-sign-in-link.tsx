import { EmailField } from "@/features/auth/components/email-field";
import { PasswordLink } from "@/features/auth/components/password-link";
import { useSendSignInLink } from "@/features/auth/data/use-send-sign-in-link";
import { hubErrors } from "@/features/auth/lib/hub-errors";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { useHubAccount } from "@/shared/hooks/use-hub-account";
import { formText } from "@/shared/lib/forms";

/** Asking the Hub to email a Sign-in Link, which goes on to `redirect` once used. */
export function SendSignInLink({ redirect }: { redirect: string | undefined }) {
  const send = useSendSignInLink();
  const account = useHubAccount();
  const errors = send.error ? hubErrors(send.error) : {};
  return (
    <SignInLayout
      heading="Email a Sign-in Link"
      action={send.isPending ? "Sending…" : "Email me a Sign-in Link"}
      pending={send.isPending}
      errors={errors}
      onSubmit={(form) => {
        send.mutate({ email: formText(form, "email"), continueTo: redirect ?? "/" });
      }}
      footer={<PasswordLink redirect={redirect} />}
    >
      <EmailField
        error={errors.email}
        defaultValue={account?.email}
        hint="It signs you in once, within 15 minutes, without a password."
      />
    </SignInLayout>
  );
}
