import { useRouter } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";

import { Notice } from "@/shared/components/sign-in-layout/notice";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** A Sign-in Link's page when the Hub couldn't look the link up. Looking it up uses nothing up. */
export function SignInLinkFailed() {
  const router = useRouter();
  return (
    <SignInCard heading="Sign in">
      <Notice kind="error">The Hub couldn’t open this Sign-in Link just now.</Notice>
      <Button
        size="lg"
        onClick={() => {
          void router.invalidate();
        }}
      >
        Try again
      </Button>
    </SignInCard>
  );
}
