import type { ComponentProps, ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { onSubmitted } from "@/lib/forms";

/** A page for getting in: setting up, or signing in. */
export function SignInPage({
  heading,
  intro,
  footer,
  children,
}: {
  heading: string;
  intro: ReactNode;
  /** Another way in, under the page's own, such as signing in with a code instead. */
  footer?: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto flex min-h-dvh w-full max-w-sm flex-col justify-center px-6 py-12">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">{heading}</h1>
      <p className="mt-2 text-muted-foreground">{intro}</p>
      <div className="mt-8">{children}</div>
      <div className="mt-6 flex justify-center empty:hidden">{footer}</div>
    </main>
  );
}

/** A `SignInPage` with one native form. */
export function SignInLayout({
  onSubmit,
  action,
  pending,
  error,
  children,
  ...page
}: Omit<ComponentProps<typeof SignInPage>, "children"> & {
  onSubmit: (form: FormData) => void;
  /** Says what submitting does, such as "Sign in". */
  action: string;
  pending: boolean;
  /** Why the Server refused the form as a whole, if it did. */
  error: string | undefined;
  children: ReactNode;
}) {
  return (
    <SignInPage {...page}>
      <form className="flex flex-col gap-5" onSubmit={onSubmitted(onSubmit)}>
        {children}
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {error}
        </p>
        <Button type="submit" disabled={pending}>
          {action}
        </Button>
      </form>
    </SignInPage>
  );
}
