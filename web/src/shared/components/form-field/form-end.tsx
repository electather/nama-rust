import type { ReactNode } from "react";

import { Button } from "@/shared/ui/button";

/** The end of a form on a page: what submitting it did, why the Server refused, and its button. */
export function FormEnd({
  action,
  variant = "default",
  pending,
  done,
  refusal,
  children,
}: {
  /** Says what submitting does, such as "Change password". */
  action: string;
  /** `ghost` for a form beside others on the page, which leaves the primary action to them. */
  variant?: "default" | "ghost";
  pending: boolean;
  /** What the last submit did, once it's done. */
  done: string | undefined;
  /** Why the Server refused the form as a whole, if it did. */
  refusal: string | undefined;
  /** Other buttons, after the submit button. */
  children?: ReactNode;
}) {
  return (
    <>
      <output className="text-sm empty:hidden">{done}</output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {refusal}
      </p>
      {/* A ghost button is pulled back by its own padding, so its words line up with the fields. */}
      <div className={variant === "ghost" ? "-ms-2.5 flex gap-2" : "flex gap-2"}>
        <Button type="submit" variant={variant} disabled={pending}>
          {action}
        </Button>
        {children}
      </div>
    </>
  );
}
