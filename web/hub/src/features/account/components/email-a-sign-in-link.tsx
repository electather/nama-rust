import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

/** Asking for a Sign-in Link, for whoever has no password or forgot it. */
export function EmailASignInLink({ continueTo }: { continueTo: string }) {
  return (
    <>
      <Link
        to="/send-sign-in-link"
        search={{ continue: continueTo }}
        className={buttonVariants({ variant: "outline", size: "lg" })}
      >
        Email me a Sign-in Link
      </Link>
      <p className="text-center text-xs text-pretty text-muted-foreground">
        Forgot your password? Sign in with a Sign-in Link, then set a new one on your Account page.
      </p>
    </>
  );
}
