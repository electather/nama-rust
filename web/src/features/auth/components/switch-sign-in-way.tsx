import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

import { OrDivider } from "@/features/auth/components/or-divider";

/** Under a sign-in form: "Or", then the link to the other way to sign in. */
export function SwitchSignInWay({
  to,
  redirect,
  label,
}: {
  to: "/sign-in" | "/sign-in/code" | "/send-sign-in-link";
  /** Where signing in goes, kept from this page. */
  redirect: string | undefined;
  label: string;
}) {
  return (
    <>
      <OrDivider />
      <Link
        to={to}
        search={{ redirect }}
        className={buttonVariants({ variant: "outline", size: "lg" })}
      >
        {label}
      </Link>
    </>
  );
}
