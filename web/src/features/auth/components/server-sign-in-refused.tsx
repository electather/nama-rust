import { Notice } from "@/shared/components/sign-in-layout/notice";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** A sign-in to a Server that can't go ahead, saying why. */
export function ServerSignInRefused({ heading, reason }: { heading: string; reason: string }) {
  return (
    <SignInCard heading={heading}>
      <Notice kind="error">{reason}</Notice>
    </SignInCard>
  );
}
