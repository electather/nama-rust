import { TextField } from "@hub/features/account/components/text-field";
import { credentialErrors } from "@hub/features/account/lib/refusals";
import { Button } from "nama-ui/components/button";
import type { ReactNode } from "react";

/** An email and a password. */
export interface Credentials {
  email: string;
  password: string;
}

/** The card's form for an email and a password, which `onSubmit` sends to the Hub. */
export function CredentialsForm({
  heading,
  action,
  attempt,
  newPassword = false,
  onSubmit,
  alternative,
  footer,
}: {
  heading: string;
  /** What the button says, such as "Sign in". */
  action: string;
  /** The attempt sending them: under way, or refused. */
  attempt: { isPending: boolean; error: Error | null };
  /** Whether this sets the password, rather than checking it. */
  newPassword?: boolean;
  onSubmit: (credentials: Credentials) => void;
  /** Another way to do it, under the button, if any. */
  alternative?: ReactNode;
  /** Another page instead, such as signing in for someone who has an Account. */
  footer: ReactNode;
}) {
  const errors = attempt.error ? credentialErrors(attempt.error) : {};
  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={(event) => {
        event.preventDefault();
        const form = new FormData(event.currentTarget);
        const text = (name: string) => {
          const value = form.get(name);
          return typeof value === "string" ? value : "";
        };
        onSubmit({ email: text("email"), password: text("password") });
      }}
    >
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">{heading}</h1>
      <TextField
        label="Email"
        name="email"
        type="email"
        autoComplete="username"
        error={errors.email}
      />
      <TextField
        label="Password"
        name="password"
        type="password"
        autoComplete={newPassword ? "new-password" : "current-password"}
        hint={newPassword ? "8 to 1024 characters." : undefined}
        error={errors.password}
      />
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors.form}
      </p>
      <Button type="submit" size="lg" disabled={attempt.isPending}>
        {action}
      </Button>
      {alternative}
      <p className="text-center text-sm text-muted-foreground">{footer}</p>
    </form>
  );
}
