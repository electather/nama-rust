import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

/** "Or", then asking for a Sign-in Link, for whoever has no password or forgot it. */
export function OrEmailASignInLink() {
  return (
    <>
      <p className="flex items-center gap-3.5 text-sm text-muted-foreground">
        <span aria-hidden className="h-px flex-1 bg-border" />
        Or
        <span aria-hidden className="h-px flex-1 bg-border" />
      </p>
      <Link to="/send-sign-in-link" className={buttonVariants({ variant: "outline", size: "lg" })}>
        Email me a Sign-in Link
      </Link>
      <p className="text-center text-xs text-pretty text-muted-foreground">
        Forgot your password? Sign in with a Sign-in Link, then set a new one on your Account page.
      </p>
    </>
  );
}
