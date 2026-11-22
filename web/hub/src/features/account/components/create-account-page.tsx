import { CredentialsForm } from "@hub/features/account/components/credentials-form";
import { useCreateAccount } from "@hub/features/account/data/sessions";
import { Link } from "@tanstack/react-router";

/** Creating an Account with an email and a password, which signs it in. */
export function CreateAccountPage() {
  const create = useCreateAccount();
  return (
    <CredentialsForm
      heading="Create an Account"
      action={create.isPending ? "Creating your Account…" : "Create Account"}
      attempt={create}
      newPassword
      onSubmit={(credentials) => {
        create.mutate(credentials);
      }}
      footer={
        <>
          Have an Account?{" "}
          <Link to="/sign-in" className="text-primary underline underline-offset-3">
            Sign in
          </Link>
        </>
      }
    />
  );
}
