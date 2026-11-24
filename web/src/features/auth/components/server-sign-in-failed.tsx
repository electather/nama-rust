import { useRouter } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";

import { Notice } from "@/shared/components/sign-in-layout/notice";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** A sign-in to a Server's page when the Hub couldn't look it up. Looking it up uses nothing up. */
export function ServerSignInFailed() {
  const router = useRouter();
  return (
    <SignInCard heading="Sign in to a Server">
      <Notice kind="error">The Hub couldn’t look up this sign-in just now.</Notice>
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
