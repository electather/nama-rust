import type { ReactNode, SubmitEvent } from "react";

import { Button } from "@/components/ui/button";

/** A page with one native form for getting in: setting up, or signing in. */
export function SignInLayout({
  heading,
  intro,
  onSubmit,
  action,
  pending,
  error,
  children,
}: {
  heading: string;
  intro: string;
  onSubmit: (form: FormData) => void;
  /** Says what submitting does, such as "Sign in". */
  action: string;
  pending: boolean;
  /** Why the Server refused the form as a whole, if it did. */
  error: string | undefined;
  children: ReactNode;
}) {
  // Handled here rather than as a form `action`, which would clear every field after a refusal.
  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit(new FormData(event.currentTarget));
  }
  return (
    <main className="mx-auto flex min-h-dvh w-full max-w-sm flex-col justify-center px-6 py-12">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">{heading}</h1>
      <p className="mt-2 text-muted-foreground">{intro}</p>
      <form className="mt-8 flex flex-col gap-5" onSubmit={submit}>
        {children}
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {error}
        </p>
        <Button type="submit" disabled={pending}>
          {action}
        </Button>
      </form>
    </main>
  );
}
