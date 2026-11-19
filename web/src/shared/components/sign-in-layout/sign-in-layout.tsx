import type { ComponentProps, ReactNode } from "react";

import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";
import { useWarp } from "@/shared/components/sign-in-layout/warp";
import { type FormErrors, onSubmitted } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/**
 * A signed-out page's card, holding one form, with the Server's refusal of the whole form above its
 * button, then `footer`. The light behind rushes while it's `pending`.
 */
export function SignInLayout({
  onSubmit,
  action,
  pending,
  errors,
  footer,
  children,
  ...page
}: Omit<ComponentProps<typeof SignInCard>, "children"> & {
  onSubmit: (form: FormData) => void;
  /** Says what submitting does, such as "Sign in". */
  action: string;
  pending: boolean;
  /** Why the Server refused: only `form` and `wait` show here; the fields show their own. */
  errors: FormErrors;
  /** Another way in, under the form, such as signing in with a code instead. */
  footer?: ReactNode;
  children: ReactNode;
}) {
  useWarp(pending);
  return (
    <SignInCard {...page}>
      <form className="flex flex-col gap-5" onSubmit={onSubmitted(onSubmit)}>
        {children}
        <FormNotices errors={errors} />
        <Button type="submit" size="lg" disabled={pending}>
          {action}
        </Button>
      </form>
      {footer}
    </SignInCard>
  );
}
