import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** A Sign-in Link that expired or was used: each works once, so offer a new one. */
export function ExpiredSignInLink() {
  return (
    <SignInCard heading="This Sign-in Link has expired">
      <p className="text-center text-base text-pretty">
        Each Sign-in Link works once, within 15 minutes. Ask for a new one to sign in.
      </p>
      <Link to="/send-sign-in-link" className={buttonVariants({ size: "lg" })}>
        Email me a new Sign-in Link
      </Link>
    </SignInCard>
  );
}
