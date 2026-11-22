import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

import { useSetupHeading } from "@/features/auth/hooks/use-setup-heading";
import { Notice } from "@/shared/components/sign-in-layout/notice";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** What setting up shows once another browser has set the Server up first. */
export function AlreadySetUp() {
  return (
    <SignInCard heading={useSetupHeading()}>
      <Notice kind="info">This Server is already set up, so you can sign in.</Notice>
      <Link to="/sign-in" className={buttonVariants({ size: "lg" })}>
        Go to sign in
      </Link>
    </SignInCard>
  );
}
