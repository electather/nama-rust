import { PageHeading } from "@hub/app/page-heading";
import { SignInLinkSent } from "@hub/features/account/components/sign-in-link-sent";
import { TextField } from "@hub/features/account/components/text-field";
import { useSendSignInLink } from "@hub/features/account/data/sign-in-links";
import { credentialErrors } from "@hub/features/account/lib/refusals";
import { Link } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";

/** Asking the Hub to email a Sign-in Link, which continues at `continueTo` once used. */
export function SendSignInLinkPage({ continueTo }: { continueTo: string }) {
  const send = useSendSignInLink();
  if (send.isSuccess) {
    return <SignInLinkSent email={send.variables.email} />;
  }
  const errors = send.error ? credentialErrors(send.error) : {};
  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={(event) => {
        event.preventDefault();
        const email = new FormData(event.currentTarget).get("email");
        send.mutate({ email: typeof email === "string" ? email : "", continueTo });
      }}
    >
      <PageHeading>Email a Sign-in Link</PageHeading>
      <TextField
        label="Email"
        name="email"
        type="email"
        autoComplete="username"
        spellCheck={false}
        enterKeyHint="send"
        hint="It signs you in once, within 15 minutes, without a password."
        error={errors.email}
      />
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors.form}
      </p>
      <Button type="submit" size="lg" disabled={send.isPending}>
        {send.isPending ? "Sending…" : "Email me a Sign-in Link"}
      </Button>
      <p className="text-center text-sm text-muted-foreground">
        <Link to="/sign-in" className="text-primary underline underline-offset-3">
          Sign in with a password
        </Link>
      </p>
    </form>
  );
}
