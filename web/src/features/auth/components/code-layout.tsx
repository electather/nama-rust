import type { ReactNode } from "react";

import { CentredUnderTopBar } from "@/features/auth/components/centred-under-top-bar";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/**
 * The card each step of signing in another Device, or confirming it’s you with an Account, shows
 * in, centred on the screen.
 */
export function CodeLayout({
  heading,
  children,
}: {
  heading?: string | undefined;
  children: ReactNode;
}) {
  return (
    <CentredUnderTopBar>
      <SignInCard heading={heading}>{children}</SignInCard>
    </CentredUnderTopBar>
  );
}
