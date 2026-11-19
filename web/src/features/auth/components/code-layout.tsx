import type { ReactNode } from "react";

import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** The card each step of signing in another Device shows in, centred on the screen. */
export function CodeLayout({ children }: { children: ReactNode }) {
  return (
    // Centred on the screen, under the top bar, which it starts below (72px, `h-18`).
    <div className="-mt-18 flex min-h-dvh items-center justify-center px-5 pt-30 pb-12">
      <SignInCard>{children}</SignInCard>
    </div>
  );
}
