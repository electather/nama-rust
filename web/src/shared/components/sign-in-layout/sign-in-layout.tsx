import type { ComponentProps, ReactNode } from "react";

import { SignInPage } from "@/shared/components/sign-in-layout/sign-in-page";
import { onSubmitted } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/** A signed-out page whose one form is the page, with the Server's refusal above its button. */
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
